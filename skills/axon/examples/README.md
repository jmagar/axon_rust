# Axon Examples

This directory contains complete, runnable example scripts demonstrating real-world Axon workflows.

## Prerequisites

1. **Credentials**: Set `FIRECRAWL_API_KEY` in `~/.claude-homelab/.env`
2. **RAG Infrastructure** (optional, for RAG examples):
   - `QDRANT_URL` - Vector database endpoint
   - `TEI_URL` - Text Embeddings Inference endpoint
   - `QDRANT_COLLECTION` - Collection name (defaults to 'axon')

See [../SKILL.md Setup section](../SKILL.md#setup) for detailed configuration.

## Examples

### 1. basic-scrape.sh

**Purpose**: Simple web page scraping workflows
**Use Cases**: Documentation extraction, blog content, static site scraping
**Complexity**: Beginner
**Runtime**: ~2 minutes

**What it demonstrates:**

- Single page scraping to markdown
- Multiple format output (markdown + HTML)
- JavaScript-heavy sites (wait-for parameter)
- Link extraction
- Auto-embedding to RAG system
- Disabling auto-embedding

**Run it:**

```bash
./skills/axon/examples/basic-scrape.sh
```

**Expected output:**

- `/tmp/axon-intro.md` - Scraped documentation
- `/tmp/axon-multi-format.json` - Multiple formats
- `/tmp/axon-links.json` - Extracted links

---

### 2. rag-pipeline.sh

**Purpose**: Complete RAG system build with semantic search
**Use Cases**: Knowledge base creation, documentation search, content discovery
**Complexity**: Intermediate
**Runtime**: ~10-15 minutes (depends on site size)

**What it demonstrates:**

- Site mapping (URL discovery)
- Full site crawling with auto-embedding
- Verifying embedded content
- Semantic search queries
- Grouped results (deduplication)
- Full document retrieval
- Multi-query aggregation
- Database management
- Incremental updates

**Requirements:**

- Qdrant running
- TEI running
- Environment variables configured

**Run it:**

```bash
./skills/axon/examples/rag-pipeline.sh
```

**Expected output:**

- `/tmp/axon-rag-pipeline/sitemap.json` - Discovered URLs
- `/tmp/axon-rag-pipeline/query*.json` - Search results
- Content embedded in Qdrant collection

---

### 3. batch-processing.sh

**Purpose**: Efficient parallel scraping of multiple URLs
**Use Cases**: Data collection, competitive analysis, content aggregation
**Complexity**: Intermediate
**Runtime**: ~5-10 minutes

**What it demonstrates:**

- Async batch scraping (start and check later)
- Status polling and monitoring
- Synchronous batch (wait for completion)
- Custom scraping options
- Results processing and extraction
- Error handling
- Batch with auto-embedding
- Reading URLs from files
- Cancelling running jobs

**Run it:**

```bash
./skills/axon/examples/batch-processing.sh
```

**Expected output:**

- `/tmp/axon-batch-processing/batch-results.json` - Batch results
- `/tmp/axon-batch-processing/all-content.md` - Combined content
- `/tmp/axon-batch-processing/url-list.txt` - Example URL list

---

### 4. monitor-website.sh

**Purpose**: Track changes to web pages over time
**Use Cases**: Documentation monitoring, pricing page alerts, ToS tracking, competitor analysis
**Complexity**: Advanced
**Runtime**: ~5 minutes (initial), then scheduled

**What it demonstrates:**

- Baseline snapshot capture
- Change detection via diff
- Detailed change analysis
- Section-specific monitoring
- Automated monitoring (cron pattern)
- Change notifications (Gotify integration)
- Historical version tracking
- Baseline updates

**Run it:**

```bash
./skills/axon/examples/monitor-website.sh
```

**Expected output:**

- `/tmp/axon-monitoring/state/*.baseline.md` - Baseline snapshots
- `/tmp/axon-monitoring/diffs/*.diff` - Change diffs
- `/tmp/axon-monitoring/alerts/*.alert.txt` - Change alerts
- `/tmp/axon-monitoring/monitor-cron.sh` - Automated monitoring script

**Schedule automated monitoring:**

```bash
crontab -e
# Add: 0 */6 * * * /tmp/axon-monitoring/monitor-cron.sh
```

---

## Running All Examples

```bash
# Run all examples in sequence
cd skills/axon/examples/
for script in *.sh; do
    echo "Running: $script"
    ./"$script"
    echo
done
```

## Customization

All examples are designed to be easily customizable:

1. **Change target URLs** - Edit the `MONITORED_PAGES` or `URLS` arrays
2. **Adjust timeouts** - Modify `--wait-for` and `--timeout` parameters
3. **Custom formats** - Use `--format markdown,html,links,screenshot`
4. **Filter content** - Add `--only-main-content`, `--include-tags`, `--exclude-tags`
5. **RAG collections** - Set `QDRANT_COLLECTION` environment variable

## Differences from scripts/

| Directory | Purpose | Audience | Completeness |
|-----------|---------|----------|--------------|
| `examples/` | Teaching and demonstration | Users learning Axon | Complete workflows with explanations |
| `scripts/` | Production operations | Skill execution (Claude Code) | Minimal, focused utilities |

**Key differences:**

1. **Examples** are self-contained tutorials with extensive comments
2. **Scripts** are production tools called by the skill during operations
3. **Examples** show multiple approaches to same problem
4. **Scripts** implement one approach efficiently
5. **Examples** are meant to be read and learned from
6. **Scripts** are meant to be executed by automation

## Learning Path

**Beginner**: Start with `basic-scrape.sh`
-> Understand scraping fundamentals and output formats

**Intermediate**: Move to `batch-processing.sh`
-> Learn parallel operations and job management

**Advanced**: Explore `rag-pipeline.sh`
-> Build complete RAG systems with semantic search

**Production**: Study `monitor-website.sh`
-> Implement automated monitoring and alerting

## Troubleshooting

### "FIRECRAWL_API_KEY must be set"

**Solution**: Add `FIRECRAWL_API_KEY=fc-...` to `~/.claude-homelab/.env`

### "RAG infrastructure not configured"

**Solution**: Set up Qdrant and TEI (see [../SKILL.md Setup](../SKILL.md#setup))

### "Command not found: axon"

**Solution**: Build the Axon CLI from source:

```bash
cargo build --release --bin axon
# Then add to your PATH or run directly:
./target/release/axon --help
```

### Rate limit errors

**Solution**: Add delays between requests or upgrade API plan

### Job timeout errors

**Solution**: Increase polling attempts in batch-processing.sh:

```bash
MAX_ATTEMPTS=60  # Wait longer
SLEEP_INTERVAL=10  # Check less frequently
```

## Next Steps

After running these examples:

1. **Read the reference docs**: See `../references/` for detailed documentation
2. **Customize for your needs**: Adapt examples to your use cases
3. **Build production workflows**: Combine patterns from multiple examples
4. **Integrate with homelab**: Use monitoring patterns with existing scripts

## Support

- **Skill documentation**: [../SKILL.md](../SKILL.md)
- **API reference**: [../references/api-endpoints.md](../references/api-endpoints.md)
- **Troubleshooting**: [../references/troubleshooting.md](../references/troubleshooting.md)
- **Commands**: [../references/commands.md](../references/commands.md)

## Contributing

When adding new examples:

1. Follow the existing script template
2. Include extensive inline comments
3. Demonstrate one clear workflow
4. Make it completely runnable
5. Add entry to this README
6. Test with fresh environment
