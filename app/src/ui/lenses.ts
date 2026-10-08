import type { Icon } from "@phosphor-icons/react";
import { AppWindow, Brain, Broom, Code, FileArchive, SquaresFour, TrendUp } from "@phosphor-icons/react";
import type { LensId } from "../design/tokens";

export const LENSES: { id: LensId; icon: Icon }[] = [
  { id: "all", icon: SquaresFour },
  { id: "junk", icon: Broom },
  { id: "apps", icon: AppWindow },
  { id: "aiModels", icon: Brain },
  { id: "dev", icon: Code },
  { id: "largeOld", icon: FileArchive },
  { id: "grew", icon: TrendUp },
];
