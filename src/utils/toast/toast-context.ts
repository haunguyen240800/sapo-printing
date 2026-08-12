import { createContext, useContext } from "react";
import { type ToastProps } from "@sapo/ui-components";

export interface ShowToastOptions extends Omit<ToastProps, "content" | "success" | "onDismiss"> {
  notify?: boolean;
  onDismiss?(): void;
  id?: string;
}

export type ShowToast = (content: string, options?: ShowToastOptions) => void;
export type ShowErrorToast = (content: string, options?: ShowToastOptions) => void;

export interface ShowToastErrorOptions extends Omit<ShowToastOptions, "notify"> {}

export interface ToastContextType {
  showToast: ShowToast;
  showErrorToast: ShowErrorToast;
}

let showToastRef: ShowToast = () => {};
export const showToast: ShowToast = (content, options) => showToastRef(content, options);

let showErrorToastRef: ShowErrorToast = () => {};
export const showErrorToast: ShowErrorToast = (content, options) => showErrorToastRef(content, options);

export const setToastRefs = (context: ToastContextType) => {
  showToastRef = context.showToast;
  showErrorToastRef = context.showErrorToast;
};

export const ToastContext = createContext<ToastContextType | undefined>(undefined);

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
