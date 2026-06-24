import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface JobStatusPayload {
  job_id: string;
  status: string;
  progress: number;
  error_message?: string;
}

type JobStatusHandler = (payload: JobStatusPayload) => void;

/**
 * Subscribe to job_status_changed Tauri events.
 * Returns unlisten function to cleanup on component unmount.
 */
export async function onJobStatusChanged(
  handler: JobStatusHandler
): Promise<UnlistenFn> {
  return listen<JobStatusPayload>('job_status_changed', (event) => {
    handler(event.payload);
  });
}
