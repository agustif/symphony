import { z } from 'zod';

// API Response Schemas
export const ThroughputSchema = z.object({
  input_tokens_per_second: z.number(),
  output_tokens_per_second: z.number(),
  total_tokens_per_second: z.number(),
  window_seconds: z.number()
});

export const ActivitySchema = z.object({
  last_poll_completed_at: z.string().nullable(),
  last_poll_started_at: z.string().nullable(),
  last_runtime_activity_at: z.string().nullable(),
  next_poll_due_at: z.string().nullable(),
  poll_in_progress: z.boolean(),
  throughput: ThroughputSchema
});

export const CodexTotalsSchema = z.object({
  input_tokens: z.number(),
  output_tokens: z.number(),
  seconds_running: z.number(),
  total_tokens: z.number()
});

export const CountsSchema = z.object({
  retrying: z.number(),
  running: z.number()
});

export const HealthSchema = z.object({
  has_rate_limits: z.boolean(),
  has_retry_backlog: z.boolean(),
  has_running_work: z.boolean(),
  last_poll_completed_at: z.number().nullable(),
  last_poll_started_at: z.number().nullable(),
  last_runtime_activity_at: z.number().nullable(),
  next_poll_due_at: z.number().nullable(),
  poll_in_progress: z.boolean(),
  status: z.string()
});

export const IssueTotalsSchema = z.object({
  active: z.number(),
  canceled: z.number(),
  completed: z.number(),
  failed: z.number(),
  pending: z.number(),
  retrying: z.number(),
  running: z.number(),
  terminal: z.number(),
  total: z.number(),
  unknown: z.number()
});

export const StateSchema = z.object({
  activity: ActivitySchema,
  codex_totals: CodexTotalsSchema,
  counts: CountsSchema,
  generated_at: z.string(),
  health: HealthSchema,
  issue_totals: IssueTotalsSchema,
  rate_limits: z.any().nullable(),
  retrying: z.array(z.any()),
  running: z.array(z.any()),
  summary: z.object({
    runtime: z.object({
      activity: z.string(),
      counts: z.string(),
      health: z.string(),
      throughput: z.string(),
      tokens: z.string()
    }),
    state: z.object({
      issues: z.string(),
      retrying_identifiers: z.array(z.string()),
      running_identifiers: z.array(z.string()),
      task_maps: z.string()
    })
  }),
  task_maps: z.object({
    inactive_tracked: z.number(),
    retrying_rows: z.number(),
    running_rows: z.number(),
    runtime_retrying_gap: z.number(),
    runtime_running_gap: z.number()
  })
});

export const RefreshResponseSchema = z.object({
  coalesced: z.boolean(),
  operations: z.array(z.string()),
  queued: z.boolean(),
  requested_at: z.string()
});

export const ErrorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string()
  })
});

export type StateResponse = z.infer<typeof StateSchema>;
export type RefreshResponse = z.infer<typeof RefreshResponseSchema>;
export type ErrorResponse = z.infer<typeof ErrorResponseSchema>;
