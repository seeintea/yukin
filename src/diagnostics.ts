import type { RootOptions } from "react-dom/client";

import {
  frontendErrorReport,
  type FrontendErrorKind,
  type FrontendErrorReportRequest,
} from "#/api/diagnostics";

const reportedErrors = new WeakMap<object, number>();

function severity(kind: FrontendErrorKind) {
  return kind === "react_caught" || kind === "react_recoverable" ? 1 : 2;
}

function errorDetails(error: unknown): Pick<FrontendErrorReportRequest, "message" | "stack"> {
  if (error instanceof Error) {
    return { message: error.message || error.name, stack: error.stack };
  }
  if (typeof error === "string") {
    return { message: error };
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = Reflect.get(error, "message");
    const stack = Reflect.get(error, "stack");
    if (typeof message === "string") {
      return { message, stack: typeof stack === "string" ? stack : undefined };
    }
  }
  return { message: "Non-Error value was thrown" };
}

function safeSource(source: string | undefined): string | undefined {
  if (!source) return undefined;

  try {
    const url = new URL(source, window.location.href);
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return undefined;
  }
}

function report(
  kind: FrontendErrorKind,
  error: unknown,
  details: Omit<FrontendErrorReportRequest, "kind" | "message" | "stack"> = {},
) {
  if (typeof error === "object" && error !== null) {
    const currentSeverity = severity(kind);
    if ((reportedErrors.get(error) ?? 0) >= currentSeverity) return;
    reportedErrors.set(error, currentSeverity);
  }

  void frontendErrorReport({ kind, ...errorDetails(error), ...details }).catch((reportError) => {
    console.error("Failed to report frontend error", reportError);
  });
}

export function installGlobalErrorHandlers() {
  window.addEventListener("error", (event) => {
    report("window", event.error ?? event.message, {
      source: safeSource(event.filename),
      line: event.lineno || undefined,
      column: event.colno || undefined,
    });
  });
  window.addEventListener("unhandledrejection", (event) => {
    report("unhandled_rejection", event.reason);
  });
}

export const reactErrorHandlers: Pick<
  RootOptions,
  "onCaughtError" | "onRecoverableError" | "onUncaughtError"
> = {
  onCaughtError(error, errorInfo) {
    report("react_caught", error, { componentStack: errorInfo.componentStack });
  },
  onRecoverableError(error, errorInfo) {
    report("react_recoverable", error, { componentStack: errorInfo.componentStack });
  },
  onUncaughtError(error, errorInfo) {
    report("react_uncaught", error, { componentStack: errorInfo.componentStack });
  },
};
