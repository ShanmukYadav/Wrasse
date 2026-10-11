//! Model weights are recognised by a header or a folder marker, not by extension alone.
//!
//! `.gguf` and `.safetensors` still have to carry their own header. `.bin` and the other
//! ambiguous extensions need one of the known headers, or a `config.json` in the same folder
//! whose JSON object has `model_type` or `architectures` (a Hugging Face snapshot's raw
//! `pytorch_model.bin` is the usual case). Only the first 8 KB of the weight file is read.
//! The config is read only up to 1 MB.

use std::fs::File;
use std::io::Read;
use std::path::Path;

const PREFIX: usize = 8192;
/// A safetensors header longer than this is not a header.
const MAX_HEADER: u64 = 100_000_000;
/// `config.json` bigger than this is not read.
const CONFIG_CAP: u64 = 1_048_576;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Header {
    Gguf,
    Safetensors,
    Pytorch,
    Onnx,
}

/// `model.safetensors` and `pytorch_model.bin` side by side, with a config that names one model.
///
/// The check is the safetensors header, the PyTorch header, and `config.json` keys. Each weight
/// file contributes at most its first 8 KB. The tensors are not read.
pub fn same_weights_two_formats(dir: &Path) -> bool {
    let safe = dir.join("model.safetensors");
    let bin = dir.join("pytorch_model.bin");
    if !present(&safe) || !present(&bin) || dataless(&safe) || dataless(&bin) {
        return false;
    }
    header(&safe) == Some(Header::Safetensors)
        && header(&bin) == Some(Header::Pytorch)
        && sibling_config(&bin)
}

fn present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|md| {
        let t = md.file_type();
        t.is_file() || t.is_symlink()
    })
}

fn dataless(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|md| crate::platform::is_dataless(&md))
}

/// True when `path` is a model weight file. No size check: callers keep their own size gate.
pub fn is_model_weights(path: &Path) -> bool {
    let Some(ext) = extension(path) else {
        return false;
    };
    match ext.as_str() {
        "gguf" => header(path) == Some(Header::Gguf),
        "safetensors" => header(path) == Some(Header::Safetensors),
        "bin" | "pt" | "pth" | "ckpt" | "onnx" | "mlmodel" | "npz" | "tflite" | "h5"
        | "mlpackage" => {
            if ext == "mlpackage" && path.is_dir() {
                return true;
            }
            header(path).is_some() || sibling_config(path)
        }
        _ => false,
    }
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

/// A sibling `config.json` counts only when it is a JSON object with `model_type` or
/// `architectures`. Any other config, including `{}`, does not make the file a model.
fn sibling_config(path: &Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    let cfg = dir.join("config.json");
    let Ok(meta) = std::fs::metadata(&cfg) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 || meta.len() > CONFIG_CAP {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(&cfg) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value
        .as_object()
        .is_some_and(|o| o.contains_key("model_type") || o.contains_key("architectures"))
}

fn header(path: &Path) -> Option<Header> {
    let mut buf = [0u8; PREFIX];
    let n = File::open(path).ok()?.read(&mut buf).ok()?;
    let bytes = &buf[..n];
    if is_gguf(bytes) {
        Some(Header::Gguf)
    } else if is_safetensors(bytes) {
        Some(Header::Safetensors)
    } else if is_pytorch(bytes) {
        Some(Header::Pytorch)
    } else if is_onnx(bytes) {
        Some(Header::Onnx)
    } else {
        None
    }
}

fn is_gguf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == b"GGUF"
}

/// Little-endian header length, then a JSON object. A file of zeros fails the length.
fn is_safetensors(bytes: &[u8]) -> bool {
    let Some(raw) = bytes.get(..8) else {
        return false;
    };
    let Ok(len) = raw.try_into() else {
        return false;
    };
    let n = u64::from_le_bytes(len);
    n > 0 && n <= MAX_HEADER && bytes.get(8) == Some(&b'{')
}

/// A PyTorch zip (`PK` local header naming a pickle) or a pickle protocol header.
fn is_pytorch(bytes: &[u8]) -> bool {
    if bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04" {
        return contains(bytes, b"data.pkl") || contains(bytes, b".pkl");
    }
    bytes.len() >= 2 && bytes[0] == 0x80 && (2..=5).contains(&bytes[1])
}

/// An ONNX protobuf starts with field 1 (tag `0x08`) and names `onnx` in the prefix.
fn is_onnx(bytes: &[u8]) -> bool {
    bytes.first() == Some(&0x08) && contains_ascii_ignore_case(bytes, b"onnx")
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn contains_ascii_ignore_case(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && hay.windows(needle.len()).any(|w| {
            w.iter()
                .zip(needle)
                .all(|(a, b)| a.to_ascii_lowercase() == *b)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-weights-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    #[test]
    fn zero_filled_bin_is_not_a_model() {
        let dir = scratch();
        let path = dir.join("old-big.bin");
        write(&path, &[0u8; 64]);
        assert!(!is_model_weights(&path));
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn gguf_magic_is_a_model_and_zeros_are_not() {
        let dir = scratch();
        let path = dir.join("mistral.gguf");
        write(&path, b"GGUF\x03\x00\x00\x00rest");
        assert!(is_model_weights(&path));
        let fake = dir.join("zeros.gguf");
        write(&fake, &[0u8; 32]);
        write(&dir.join("config.json"), br#"{"model_type":"llama"}"#);
        assert!(
            !is_model_weights(&fake),
            "a .gguf still needs the GGUF header"
        );
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn safetensors_header_is_a_model_and_filler_bytes_are_not() {
        let dir = scratch();
        let path = dir.join("adapter.safetensors");
        let json = br#"{"format":"pt"}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json);
        write(&path, &bytes);
        assert!(is_model_weights(&path));
        let fake = dir.join("zeros.safetensors");
        write(&fake, &[1u8; 32]);
        assert!(!is_model_weights(&fake));
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn bin_needs_a_header_or_a_model_config() {
        let dir = scratch();
        let path = dir.join("pytorch_model.bin");
        write(&path, &[0u8; 32]);
        assert!(!is_model_weights(&path));
        write(&dir.join("config.json"), b"{}");
        assert!(!is_model_weights(&path), "an empty config is not a model");
        write(&dir.join("config.json"), br#"{"model_type":"bert"}"#);
        assert!(is_model_weights(&path));
        write(
            &dir.join("config.json"),
            br#"{"architectures":["BertModel"]}"#,
        );
        assert!(is_model_weights(&path));
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn a_bin_beside_a_non_model_config_json_is_not_a_model() {
        let dir = scratch();
        let path = dir.join("weights.bin");
        write(&path, &[0u8; 32]);
        write(&dir.join("config.json"), br#"{"name":"not-a-model"}"#);
        assert!(
            !is_model_weights(&path),
            "a config without model_type or architectures is not a model"
        );
        write(&dir.join("config.json"), br#"{"note":"model_type"}"#);
        assert!(
            !is_model_weights(&path),
            "those words in a value do not count"
        );
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn two_formats_need_both_headers_and_a_model_config() {
        let dir = scratch();
        let safe = dir.join("model.safetensors");
        let json = br#"{"format":"pt"}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json);
        write(&safe, &bytes);
        write(&dir.join("pytorch_model.bin"), b"PK\x03\x04data.pkl");
        assert!(!same_weights_two_formats(&dir), "no model config yet");
        write(&dir.join("config.json"), b"{}");
        assert!(
            !same_weights_two_formats(&dir),
            "an empty config is not enough"
        );
        write(&dir.join("config.json"), br#"{"model_type":"deberta"}"#);
        assert!(same_weights_two_formats(&dir));
        write(&dir.join("pytorch_model.bin"), &[0u8; 32]);
        assert!(
            !same_weights_two_formats(&dir),
            "a zero-filled bin is not the pytorch copy"
        );
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn pytorch_and_onnx_headers_count_and_zeros_do_not() {
        let dir = scratch();
        let pt = dir.join("model.pt");
        write(&pt, b"\x80\x02rest");
        assert!(is_model_weights(&pt));
        let zip = dir.join("w.bin");
        write(&zip, b"PK\x03\x04data.pkl");
        assert!(is_model_weights(&zip));
        let onnx = dir.join("m.onnx");
        let mut body = vec![0x08, 0x01];
        body.extend_from_slice(b"onnx");
        write(&onnx, &body);
        assert!(is_model_weights(&onnx));
        let zero = dir.join("z.onnx");
        write(&zero, &[0u8; 16]);
        assert!(!is_model_weights(&zero));
        crate::platform::remove_test_dir(&dir);
    }
}
