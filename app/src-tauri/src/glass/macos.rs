//! AppKit glass views placed under the WKWebView. Runs on the main thread only
//! (Tauri's `with_webview` callback runs there).

use super::{Anchor, GlassKind, Region};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

const BACKGROUND: &str = "__background";

thread_local! {
    static VIEWS: RefCell<HashMap<String, Retained<NSView>>> = RefCell::new(HashMap::new());
}

/// NSGlassEffectView exists from macOS 26; look it up at runtime so older systems never touch it.
pub fn detect() -> GlassKind {
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        GlassKind::Liquid
    } else {
        GlassKind::Vibrancy
    }
}

fn webview_view(ptr: *mut c_void) -> Option<&'static NSView> {
    // SAFETY: Tauri hands us its live WKWebView (an NSView subclass) on the main thread.
    unsafe { (ptr as *const NSView).as_ref() }
}

fn make_view(
    kind: GlassKind,
    mtm: MainThreadMarker,
    frame: NSRect,
    radius: f64,
    background: bool,
) -> Retained<NSView> {
    match kind {
        GlassKind::Liquid => {
            let view = NSGlassEffectView::initWithFrame(mtm.alloc(), frame);
            view.setStyle(if background {
                NSGlassEffectViewStyle::Clear
            } else {
                NSGlassEffectViewStyle::Regular
            });
            view.setCornerRadius(radius);
            Retained::into_super(view)
        }
        _ => {
            let view = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
            view.setMaterial(if background {
                NSVisualEffectMaterial::UnderWindowBackground
            } else {
                NSVisualEffectMaterial::HUDWindow
            });
            view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            view.setState(NSVisualEffectState::Active);
            view.setWantsLayer(true);
            // SAFETY: layer-backed after setWantsLayer; CALayer responds to these setters.
            unsafe {
                let layer: *mut AnyObject = msg_send![&*view, layer];
                if !layer.is_null() {
                    let _: () = msg_send![layer, setCornerRadius: radius];
                    let _: () = msg_send![layer, setMasksToBounds: true];
                }
            }
            Retained::into_super(view)
        }
    }
}

pub fn install(ptr: *mut c_void, kind: GlassKind) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(webview) = webview_view(ptr) else {
        return;
    };
    // SAFETY: reading the superview of a live view on the main thread.
    let Some(parent) = (unsafe { webview.superview() }) else {
        return;
    };
    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        if views.contains_key(BACKGROUND) {
            return;
        }
        let view = make_view(kind, mtm, parent.bounds(), 0.0, true);
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        parent.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, Some(webview));
        views.insert(BACKGROUND.into(), view);
        if cfg!(debug_assertions) {
            println!(
                "[glass] installed {kind:?} background under the webview ({}x{})",
                parent.bounds().size.width,
                parent.bounds().size.height
            );
        }
    });
}

pub fn set_regions(ptr: *mut c_void, kind: GlassKind, regions: &[Region]) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(webview) = webview_view(ptr) else {
        return;
    };
    // SAFETY: as above.
    let Some(parent) = (unsafe { webview.superview() }) else {
        return;
    };
    let wv = webview.frame();
    let flipped = parent.isFlipped();

    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        views.retain(|id, view| {
            let keep = id == BACKGROUND || regions.iter().any(|r| &r.id == id);
            if !keep {
                view.removeFromSuperview();
            }
            keep
        });

        for r in regions {
            let y = if flipped {
                wv.origin.y + r.y
            } else {
                wv.origin.y + wv.size.height - r.y - r.h
            };
            let frame = NSRect::new(NSPoint::new(wv.origin.x + r.x, y), NSSize::new(r.w, r.h));
            // Keep the control's distance to its anchored edge during live resize, centered horizontally.
            let top_is_min_y = flipped;
            let flexible_vertical = match (&r.anchor, top_is_min_y) {
                (Anchor::Top, true) | (Anchor::Bottom, false) => {
                    NSAutoresizingMaskOptions::ViewMaxYMargin
                }
                (Anchor::Top, false) | (Anchor::Bottom, true) => {
                    NSAutoresizingMaskOptions::ViewMinYMargin
                }
            };
            let mask = NSAutoresizingMaskOptions::ViewMinXMargin
                | NSAutoresizingMaskOptions::ViewMaxXMargin
                | flexible_vertical;

            if let Some(view) = views.get(&r.id) {
                view.setFrame(frame);
                if kind == GlassKind::Liquid {
                    if let Ok(glass) = view.clone().downcast::<NSGlassEffectView>() {
                        glass.setCornerRadius(r.radius);
                    }
                }
            } else {
                let view = make_view(kind, mtm, frame, r.radius, false);
                view.setAutoresizingMask(mask);
                parent.addSubview_positioned_relativeTo(
                    &view,
                    NSWindowOrderingMode::Below,
                    Some(webview),
                );
                views.insert(r.id.clone(), view);
            }
        }
        if cfg!(debug_assertions) {
            let ids: Vec<_> = regions
                .iter()
                .map(|r| format!("{}@{:.0},{:.0} {:.0}x{:.0}", r.id, r.x, r.y, r.w, r.h))
                .collect();
            println!(
                "[glass] {kind:?} regions: [{}], parent flipped={flipped}",
                ids.join(", ")
            );
        }
    });
}
