/**
 * DTO cho print job từ backend (Rust).
 * Matches: src-tauri/src/application/dto/job_dto.rs::JobDto
 */
export interface JobDto {
  job_id: string;
  printer_name: string;
  status: string;
  progress: number; // 0-100
  created_at: number; // Unix timestamp
  error_message?: string;
}

/**
 * DTO cho filtering danh sách jobs.
 * Matches: src-tauri/src/application/dto/job_dto.rs::JobFilterDto
 */
export interface JobFilterDto {
  status?: string;
  printer_name?: string;
  from_date?: number; // Unix timestamp
  to_date?: number; // Unix timestamp
}
