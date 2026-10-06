import type { AppError } from "../bindings";

export type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };
