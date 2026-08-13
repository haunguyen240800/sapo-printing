export interface Job {
  job_id: string;
  printer_name: string;
  status: string;
  progress: number;
  created_at: number;
  error_message?: string;
}

export interface JobFilter {
  status?: string;
  printer_name?: string;
  from_date?: number;
  to_date?: number;
}

