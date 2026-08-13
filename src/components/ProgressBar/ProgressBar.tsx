import { CSSProperties, useRef } from "react";
import { Transition, type TransitionStatus } from "react-transition-group";
import styled from "@emotion/styled";
import { useTheme } from "@sapo/ui-components";
import { visuallyHidden } from "src/utils/styles";

type Tone = "primary" | "success" | "critical";

export interface ProgressBarProps {
  /**
   * Tiến trình của task
   * @default 0
   */
  progress?: number;
  /**
   * Kích thước của progressbar
   * @default 'medium'
   */
  size?: "small" | "medium";
  /**
   * Màu sắc của progressbar
   * @default 'primary'
   */
  tone?: Tone;
  /**
   * Có hiệu ứng animation hay không
   * @default true
   */
  animated?: boolean;
}

const PROGRESS_MAX = 100;

/**
 * Thường được dùng để mô tả tiến trình chạy.
 */
export function ProgressBar({
  animated = true,
  progress: progressProp = 0,
  size = "medium",
  tone: toneProp,
}: ProgressBarProps) {
  const tone: Tone = toneProp ?? "primary";
  const progress = Math.max(0, Math.min(progressProp, PROGRESS_MAX));
  const indicatorRef = useRef<HTMLDivElement>(null);
  const theme = useTheme();

  const progressBarDuration = animated ? theme.motion.duration300 : theme.motion.duration0;

  const transitionStyles: Record<TransitionStatus, CSSProperties> = {
    entering: { transform: `scaleX(0)` },
    entered: { transform: `scaleX(${progress}%)` },
    exiting: { transform: `scaleX(${progress}%)` },
    exited: { transform: `scaleX(0)` },
    unmounted: {},
  };

  return (
    <StyledProgressBar size={size}>
      <StyledProgress value={progress} max={PROGRESS_MAX} />
      <Transition appear in timeout={parseInt(progressBarDuration, 10)} nodeRef={indicatorRef}>
        {(state) => (
          <StyledIndicator ref={indicatorRef} animated={animated} $tone={tone} style={transitionStyles[state]}>
            <StyledLabel>{progress}%</StyledLabel>
          </StyledIndicator>
        )}
      </Transition>
    </StyledProgressBar>
  );
}

const StyledProgressBar = styled.div<{
  size: ProgressBarProps["size"];
}>`
  overflow: hidden;
  width: 100%;
  background-color: #d2d6db;
  border-radius: ${(p) => p.theme.shape.borderRadius("base")};
  height: ${(p) => (p.size === "small" ? p.theme.spacing(2) : p.theme.spacing(4))};
`;

const StyledProgress = styled.progress`
  ${visuallyHidden}
`;

const StyledIndicator = styled.div<{
  animated: boolean;
  $tone: Tone;
}>`
  height: inherit;
  background-color: ${(p) => {
    switch (p.$tone) {
      case "success":
        return p.theme.colors.borderSuccess;
      case "critical":
        return p.theme.colors.actionCritical;
      default:
        return "#156EEB";
    }
  }};
  transform-origin: 0 50%;
  transition: transform ${(p) => (p.animated ? p.theme.motion.duration300 : p.theme.motion.duration0)}
    ${(p) => p.theme.motion.transformEaseInOut};
`;

const StyledLabel = styled.span`
  ${visuallyHidden}
`;
