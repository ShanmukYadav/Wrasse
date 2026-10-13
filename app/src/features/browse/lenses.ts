import type { IconName } from "../../design/icons/nodes";
import type { CategoryId, FilterId, LensId } from "../../design/tokens";

export const CATEGORIES: { id: CategoryId; icon: IconName }[] = [
  { id: "all", icon: "all" },
  { id: "junk", icon: "junk" },
  { id: "apps", icon: "app" },
  { id: "aiModels", icon: "aiModel" },
  { id: "dev", icon: "dev" },
];

export const FILTERS: { id: FilterId; icon: IconName }[] = [
  { id: "largeOld", icon: "largeOld" },
  { id: "grew", icon: "grew" },
];

/** Categories and filters, for command icons. */
export const LENSES: { id: LensId; icon: IconName }[] = [...CATEGORIES, ...FILTERS];
