export function getErrorMessage(error: unknown, fallback = "请稍后重试") {
  if (typeof error === "string" && error) {
    return error;
  }

  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return fallback;
}
