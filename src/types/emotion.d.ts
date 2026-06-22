import type { Theme as SapoTheme } from "@sapo/ui-components";

import "@emotion/react";

declare module "@emotion/react" {
  export interface Theme extends SapoTheme {}
}
