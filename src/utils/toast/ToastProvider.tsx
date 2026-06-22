import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Toast, type ToastProps } from "@sapo/ui-components";

import { setToastRefs, type ShowToastOptions, ToastContext, type ToastContextType } from "./toast-context";

interface Props {
  children: ReactNode;
  disabled?: boolean;
}

const DEFAULT_ID = "DEFAULT";

/** TODO: fix multi toast */
export const ToastProvider = ({ children, disabled = false }: Props) => {
  const [toasts, setToasts] = useState<Record<string, ToastProps>>({});
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;

  useEffect(() => {
    if (disabled) setToasts({});
  }, [disabled]);

  const addToast = useCallback((content: string, options?: ShowToastOptions) => {
    if (disabledRef.current) return;

    const { id = DEFAULT_ID, ...restOptions } = options || {};
    setToasts((toasts) => ({
      ...toasts,
      [id]: {
        ...restOptions,
        success: !restOptions.error && !restOptions.notify,
        error: restOptions.error,
        content,
        onDismiss: () => {
          setToasts(({ [id]: _, ...remain }) => remain);
          restOptions.onDismiss?.();
        },
      },
    }));
  }, []);

  const context = useMemo(
    (): ToastContextType => ({
      showToast: addToast,
      showErrorToast: (content, options) => addToast(content, { ...options, error: true }),
    }),
    [addToast]
  );
  setToastRefs(context);

  return (
    <>
      {!disabled &&
        Object.entries(toasts).map(([id, toast]) => <Toast key={id === DEFAULT_ID ? toast.content : id} {...toast} />)}
      <ToastContext.Provider value={context}>{children}</ToastContext.Provider>
    </>
  );
};
