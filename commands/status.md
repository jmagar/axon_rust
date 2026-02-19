---
description: Check status of Axon jobs (crawls, batches, extracts)
argument-hint: [job-id]
allowed-tools: Bash(axon *)
---

# Job Status Check

Execute the Axon status command with the provided arguments:

```bash
axon status $ARGUMENTS --json
```

## Instructions

1. **Execute the command** using the Bash tool with the arguments provided
2. **Parse the response** based on query type:
   - **Specific job**: Detailed status for single job ID
   - **All jobs**: Summary of all running/completed jobs
3. **Extract status information**:
   - Job ID and type (crawl/batch/extract/embed)
   - Current status (queued/running/completed/failed)
   - Progress percentage
   - Completed items / Total items
   - Errors or warnings
   - Results (if completed)
4. **Present the results** as:
   - Status overview table (for all jobs)
   - Detailed progress report (for specific job)
   - Error details (if any failures)
   - Next action recommendations

## Expected Output

The command returns JSON containing:
- `jobs`: Array of job status objects:
  - `job_id`: Job identifier
  - `type`: Job type (crawl/batch/extract/embed)
  - `status`: Current status
  - `progress`: Completion percentage
  - `completed`: Items completed
  - `total`: Total items
  - `errors`: Error messages (if any)
  - `results`: Results data (if completed)

Present job status with progress indicators and completion details.
