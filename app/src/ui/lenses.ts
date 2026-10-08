import type { IconName } from "../design/icons/nodes";
import type { LensId } from "../design/tokens";

export const LENSES: { id: LensId; icon: IconName }[] = [
  { id: "all", icon: "all" },
  { id: "junk", icon: "junk" },
  { id: "apps", icon: "app" },
  { id: "aiModels", icon: "aiModel" },
  { id: "dev", icon: "dev" },
  { id: "largeOld", icon: "largeOld" },
  { id: "grew", icon: "grew" },
];
