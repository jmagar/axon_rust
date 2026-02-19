# Axon Job Management

Complete reference for managing asynchronous Axon jobs (crawl, batch, extract, embed).

---

## Understanding Job Types

Axon supports four types of asynchronous jobs:

1. **Crawl Jobs** - Website crawling operations
2. **Batch Jobs** - Batch scraping of multiple URLs
3. **Extract Jobs** - Structured data extraction
4. **Embed Jobs** - Vector embedding operations

All jobs can run **asynchronously** (returns job ID immediately) or **synchronously** (waits for completion with `--wait` flag).

---

## List Active Crawls

List all active crawl jobs currently running.

```bash
# List all active crawl jobs
axon list

# Save to file
axon list -o crawls.json
```

**Parameters:**
- `-o, --output <path>`: Save output to file
- `--no-pretty`: Disable pretty JSON output

**Output:**
```json
{
  "crawls": [
    {
      "id": "abc123def456",
      "url": "https://example.com",
      "status": "scraping",
      "total": 150,
      "completed": 47,
      "creditsUsed": 47
    }
  ]
}
```

---

## Check Job Status

Show active jobs and embedding queue status. This is the **unified status command** for all job types.

### Check All Jobs

```bash
# Show overview of all active jobs
axon status

# Output as JSON
axon status --json

# Pretty JSON output
axon status --pretty
```

**Output:**
```json
{
  "crawls": {
    "active": 2,
    "jobs": [...]
  },
  "batches": {
    "active": 1,
    "jobs": [...]
  },
  "extracts": {
    "active": 0
  },
  "embedQueue": {
    "pending": 3,
    "processing": 1
  }
}
```

---

### Check Specific Crawl Jobs

```bash
# Check specific crawl jobs by ID
axon status --crawl job-id-1,job-id-2

# Single job
axon status --crawl abc123def456
```

**Output:**
```json
{
  "job-id-1": {
    "status": "scraping",
    "total": 100,
    "completed": 45,
    "creditsUsed": 45,
    "expiresAt": "2026-02-06T12:00:00Z"
  }
}
```

**Possible statuses:**
- `scraping`: Job in progress
- `completed`: Job finished successfully
- `failed`: Job encountered error

---

### Check Specific Batch Jobs

```bash
# Check specific batch jobs by ID
axon status --batch job-id-1,job-id-2

# Single batch job
axon status --batch xyz789abc012
```

**Output:**
```json
{
  "job-id-1": {
    "status": "scraping",
    "total": 50,
    "completed": 30,
    "failed": 2,
    "data": [...]
  }
}
```

---

### Check Specific Extract Jobs

```bash
# Check specific extract jobs by ID
axon status --extract job-id-1,job-id-2

# Single extract job
axon status --extract def456ghi789
```

**Output:**
```json
{
  "job-id-1": {
    "status": "completed",
    "data": {...},
    "warning": "Limited to 100 pages"
  }
}
```

---

### Check Embedding Queue

```bash
# Show embedding queue status
axon status --embed

# Check specific embedding job
axon status --embed job-id
```

**Output (queue overview):**
```json
{
  "queue": {
    "pending": 5,
    "processing": 2,
    "completed": 143,
    "failed": 1
  },
  "jobs": [
    {
      "id": "embed-123",
      "url": "https://example.com/doc",
      "status": "pending",
      "chunks": 15
    }
  ]
}
```

**Output (specific job):**
```json
{
  "job-id": {
    "status": "completed",
    "url": "https://example.com/doc",
    "chunks": 15,
    "collection": "axon",
    "timestamp": "2026-02-06T10:30:00Z"
  }
}
```

---

## Crawl Job Management

### Start Async Crawl

```bash
# Start crawl (returns job ID immediately)
axon crawl https://example.com --limit 100

# Output:
# Crawl job started: abc123def456
# Status: axon status --crawl abc123def456
```

---

### Wait for Crawl Completion

```bash
# Synchronous crawl (waits for completion)
axon crawl https://example.com --limit 100 --wait

# With progress indicator
axon crawl https://example.com --limit 100 --wait --progress

# Custom poll interval
axon crawl https://example.com --wait --poll-interval 5000
```

**Parameters:**
- `--wait`: Wait for completion (synchronous mode)
- `--progress`: Show progress bar (requires `--wait`)
- `--poll-interval <ms>`: Status check interval (default: 2000ms)

---

### Check Crawl Status

```bash
# Check specific crawl
axon status --crawl abc123def456

# Check multiple crawls
axon status --crawl job1,job2,job3
```

---

## Batch Job Management

### Start Batch Job

```bash
# Start batch scrape (async)
axon batch url1 url2 url3

# Synchronous batch (wait for completion)
axon batch url1 url2 url3 --wait

# With custom poll interval
axon batch url1 url2 --wait --poll-interval 10
```

**Returns:**
```json
{
  "success": true,
  "id": "batch-xyz789"
}
```

---

### Check Batch Status

```bash
# Check batch status
axon batch status <job-id>

# Or use unified status command
axon status --batch <job-id>
```

**Output:**
```json
{
  "status": "scraping",
  "total": 100,
  "completed": 67,
  "failed": 2,
  "creditsUsed": 67,
  "expiresAt": "2026-02-06T15:00:00Z",
  "data": [
    {
      "url": "https://example.com/page1",
      "markdown": "...",
      "status": "completed"
    }
  ]
}
```

---

### Cancel Batch Job

```bash
# Cancel running batch job
axon batch cancel <job-id>
```

**Output:**
```json
{
  "success": true,
  "message": "Batch job cancelled"
}
```

---

### Get Batch Errors

```bash
# Get errors for batch job
axon batch errors <job-id>
```

**Output:**
```json
{
  "errors": [
    {
      "url": "https://example.com/page1",
      "error": "Timeout after 15s",
      "statusCode": 408
    },
    {
      "url": "https://example.com/page2",
      "error": "Page not found",
      "statusCode": 404
    }
  ]
}
```

---

## Extract Job Management

### Start Extract Job

```bash
# Start extraction (async)
axon extract https://example.com --prompt "Extract data"

# Check status later
axon extract status <job-id>
```

---

### Check Extract Status

```bash
# Check extract job status
axon extract status <job-id>

# Or use unified status command
axon status --extract <job-id>
```

**Output:**
```json
{
  "status": "completed",
  "data": {
    "products": [
      {"name": "Product 1", "price": 29.99},
      {"name": "Product 2", "price": 49.99}
    ]
  },
  "warning": "Limited to 100 pages"
}
```

---

## Embedding Job Management

### Start Embedding Job

```bash
# Embed URL (async by default)
axon embed https://example.com

# Embed from file
axon embed --url https://example.com < document.md

# Embed from stdin
cat document.md | axon embed --url https://example.com
```

**Returns job ID:**
```json
{
  "success": true,
  "jobId": "embed-abc123",
  "message": "Embedding job queued"
}
```

---

### Check Embedding Status

```bash
# Show embedding queue overview
axon status --embed

# Check specific embedding job
axon status --embed embed-abc123
```

**Output:**
```json
{
  "status": "completed",
  "url": "https://example.com/doc",
  "chunks": 23,
  "collection": "axon",
  "vectorsCreated": 23,
  "timestamp": "2026-02-06T10:45:00Z"
}
```

**Possible statuses:**
- `pending`: In queue, waiting to process
- `processing`: Currently embedding
- `completed`: Successfully embedded
- `failed`: Embedding failed

---

### Cancel Embedding Job

```bash
# Cancel pending embedding job
axon embed cancel <job-id>
```

**Note:** Can only cancel jobs in `pending` status, not `processing` or `completed`.

---

## Job Lifecycle

### Typical Async Job Flow

1. **Start Job** - Returns job ID immediately
2. **Poll Status** - Check job status periodically
3. **Job Completes** - Status changes to `completed`
4. **Retrieve Results** - Get final results from job

**Example:**
```bash
# 1. Start crawl
JOB_ID=$(axon crawl https://example.com --limit 50 | jq -r '.id')

# 2. Poll status (in loop)
while true; do
    STATUS=$(axon status --crawl "$JOB_ID" --json | jq -r ".\"$JOB_ID\".status")
    echo "Status: $STATUS"
    [ "$STATUS" = "completed" ] && break
    sleep 5
done

# 3. Job completed - results available
echo "Crawl completed!"
```

---

### Typical Sync Job Flow

1. **Start Job with --wait** - Blocks until completion
2. **Job Completes** - Returns results directly

**Example:**
```bash
# Single command, waits for completion
axon crawl https://example.com --limit 50 --wait --progress
# Returns final results when done
```

---

## Job Expiration

Jobs have expiration times after which results are deleted:

- **Crawl jobs**: Typically expire after 24 hours
- **Batch jobs**: Expire based on plan limits
- **Extract jobs**: Expire after completion
- **Embed jobs**: Results stored permanently in vector DB

Check `expiresAt` field in status output to see when job results expire.

**Example:**
```json
{
  "status": "completed",
  "expiresAt": "2026-02-07T10:00:00Z",
  "data": [...]
}
```

---

## Error Handling

### Common Job Errors

**Timeout:**
```json
{
  "error": "Job timeout after 30 minutes",
  "statusCode": 408
}
```
**Solution:** Reduce crawl scope with `--limit` or `--max-depth`

**Rate Limit:**
```json
{
  "error": "Rate limit exceeded",
  "statusCode": 429
}
```
**Solution:** Add `--delay` between requests

**Invalid Job ID:**
```json
{
  "error": "Job not found",
  "statusCode": 404
}
```
**Solution:** Verify job ID, check if job expired

---

## Best Practices

### For Long Crawls
- Use async mode (no `--wait`)
- Poll status every 5-10 seconds
- Store job ID for later retrieval
- Set up webhook for completion notification

### For Quick Operations
- Use sync mode (`--wait`)
- Add `--progress` for visual feedback
- Results returned immediately

### For Batch Jobs
- Group related URLs together
- Use `--webhook` for completion notification
- Check errors with `axon batch errors <job-id>`
- Consider `--idempotency-key` to prevent duplicates

### For Embedding Jobs
- Monitor queue with `axon status --embed`
- Large documents create multiple jobs
- Use `--collection` to organize embeddings
- Check completion before querying

---

## Troubleshooting

**Job stuck in 'scraping' status:**
- Wait longer (large crawls take time)
- Check API status with `axon --status`
- Verify network connectivity

**Job not found:**
- Job may have expired (check expiration time)
- Verify job ID is correct
- Use `axon list` to see active jobs

**Slow job progress:**
- Reduce `--delay` if set too high
- Increase `--max-concurrency` if allowed
- Check website responsiveness
- Consider using `--max-depth` to limit scope

**Failed chunks in embedding:**
- Check TEI service is running
- Verify `TEI_URL` in `.env` is correct
- Check Qdrant connection
- Review embedding job errors

---

For command reference, see [commands.md](./commands.md)
For parameter details, see [parameters.md](./parameters.md)
For RAG features, see [vector-database.md](./vector-database.md)
