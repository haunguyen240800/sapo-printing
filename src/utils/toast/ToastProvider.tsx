import React, { useCallback, useContext, useMemo, useState } from "react";
import { Toast, type ToastProps } from "@sapo/ui-components";

export interface ShowToastOptions extends Omit<ToastProps, "content" | "success" | "onDismiss"> {
  notify?: boolean;
  onDismiss?(): void;
  id?: string;
}

type ShowToast = (content: string, options?: ShowToastOptions) => void;
type ShowErrorToast = (content: string, options?: ShowToastOptions) => void;

export interface ShowToastErrorOptions extends Omit<ShowToastOptions, "notify"> {}

export interface ToastContextType {
  showToast: ShowToast;
  showErrorToast: ShowErrorToast;
}

let showToastRef: ShowToast = () => {};
export const showToast: ShowToast = (content, options) => showToastRef(content, options);

let showErrorToastRef: ShowErrorToast = () => {};
export const showErrorToast: ShowErrorToast = (content, options) => showErrorToastRef(content, options);

export const ToastContext = React.createContext<ToastContextType | undefined>(undefined);

interface Props {
  children: React.ReactNode;
}

const DEFAULT_ID = "DEFAULT";

/** TODO: fix multi toast */
export const ToastProvider = ({ children }: Props) => {
  const [toasts, setToasts] = useState<{ [key: string]: ToastProps }>({});

  const addToast = useCallback((content: string, options?: ShowToastOptions) => {
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
  showToastRef = context.showToast;
  showErrorToastRef = context.showErrorToast;

  return (
    <>
      {Object.entries(toasts).map(([id, toast]) => (
        <Toast key={id === DEFAULT_ID ? toast.content : id} {...toast} />
      ))}
      <ToastContext.Provider value={context}>{children}</ToastContext.Provider>
    </>
  );
};

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error("Missing Toast context");
  }

  return {
    showToast: context.showToast,
    showErrorToast: context.showErrorToast,
  };
}
