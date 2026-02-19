---
description: Batch scrape multiple URLs with job management
argument-hint: <url1> <url2> ... [options] | status <job-id> | cancel <job-id>
allowed-tools: Bash(axon *)
---

# Batch Scrape Multiple URLs

Execute the Axon batch command with the provided arguments:

```bash
axon batch $ARGUMENTS
```

## Instructions

1. **Execute the command** using the Bash tool with the arguments provided
2. **Handle different batch operations**:
   - **Scrape multiple URLs**: Process array of URLs asynchronously
   - **Check status**: Query job status with `status <job-id>`
   - **Cancel job**: Stop running job with `cancel <job-id>`
3. **Parse the response** based on operation:
   - **New batch**: Job ID and initial status
   - **Status check**: Progress, completed URLs, errors
   - **Cancel**: Cancellation confirmation
4. **Present the results** including:
   - Job ID for tracking
   - Number of URLs processed/pending
   - Summary of scraped content
   - Any errors or failures

## Expected Output

By default, output is human-readable text. Add `--json` for machine-readable output.

With `--json`, the command returns JSON containing:
- `id`: Batch job identifier
- `status`: Current status (queued/running/completed/failed)
- `total`: Total URLs in batch
- `completed`: Number of URLs processed
- `results`: Array of scraped content (when complete)

Present batch processing status and any per-URL failures.
