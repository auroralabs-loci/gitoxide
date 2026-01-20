# Mirror Template Setup Guide

This is a GitHub template repository for mirroring an upstream repository and syncing its pull requests. Follow these steps to set up the mirror with your own upstream repository.

## Initial Setup

1. **Create a new repository from this template**
   - Click "Use this template" on GitHub
   - Choose a name and description for your mirror repository


## Configure Repository Variables

Add these variables in your repository settings under **Settings → Secrets and variables → Variables**:

| Variable | Description | Example |
|----------|-------------|---------|
| `UPSTREAM_REPO` | The upstream repository to mirror | `owner/repo-name` |
| `MIRROR_MAX_UPSTREAM_PRS` | Maximum number of upstream PRs to mirror | `2` |
| `MIRROR_UPSTREAM_PR_LOOKBACK_DAYS` | Consider PRs from the last N days | `7` |
| `LOCI_ENV` | LOCI environment name | `PROD__AL_DEMO` |

## Configure Repository Secrets

Add these secrets in your repository settings under **Settings → Secrets and variables → Secrets**:

| Secret | Description |
|--------|-------------|
| `MIRROR_REPOS_WRITE_PAT` | Personal Access Token with `repo` and `workflow` scopes. **Note**: If this secret already exists in your organization, you can reuse the existing token. |

## Configure Environment Variables

Create a new environment in your repository settings under **Settings → Environments → New environment**:

1. Name it (e.g., `PROD__AL_DEMO`)
2. Add these secrets and variables within the environment:

| Item | Type | Description |
|--------|------|-------------|
| `LOCI_API_KEY` | Secret | API key for LOCI analysis service |
| `LOCI_BACKEND_URL` | Variable | LOCI backend URL for analysis (e.g., `https://prod.api.loci-dev.net/`) |

## Workflows Overview

### 1. **Sync Upstream** ([`.github/workflows/sync-upstream.yml`](.github/workflows/sync-upstream.yml))
- **Schedule**: Every 4 hours
- **Purpose**: Syncs the upstream repository's default branch to your `main` branch and applies overlays from the `overlay` branch

### 2. **Mirror Upstream PRs** ([`.github/workflows/sync-upstream-prs.yml`](.github/workflows/sync-upstream-prs.yml))
- **Schedule**: Multiple times daily (1:30, 5:30, 9:30, 13:30, 17:30, 21:30 UTC)
- **Purpose**: Automatically mirrors open pull requests from the upstream repository

### 3. **LOCI Analysis** ([`.github/workflows/loci-analysis.yml`](.github/workflows/loci-analysis.yml))
- **Triggers**: On pushes to `main*` branches and pull requests
- **Purpose**: Runs LOCI analysis on the Rust project (requires environment secrets: `LOCI_API_KEY` and `LOCI_BACKEND_URL`)

## Verify Everything Works

1. Go to **Actions** tab in your repository
2. Manually trigger **"Sync Upstream"** workflow to test
3. Check that your `main` branch updates with upstream content
4. Review workflow logs for any errors

That's it! Your mirror is now set up and will automatically sync