import { css } from "@emotion/react";

export const visuallyHidden = css`
  position: absolute !important;
  top: 0;
  width: 1px !important;
  height: 1px !important;
  margin: 0 !important;
  padding: 0 !important;
  overflow: hidden !important;
  clip-path: inset(50%) !important;
  border: 0 !important;
  white-space: nowrap !important;
`;
