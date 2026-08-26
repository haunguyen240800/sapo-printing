import { ComponentType, ReactNode, Suspense } from "react";

export function withSuspense(Component: ComponentType, fallback: ReactNode = null) {
  return (
    <Suspense fallback={fallback}>
      <Component />
    </Suspense>
  );
}
