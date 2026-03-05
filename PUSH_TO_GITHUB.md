# 🚀 Push SpecterOS to GitHub

## Step 1: Create GitHub Repository

1. Go to **https://github.com/new**
2. Repository name: **`specteros`** or **`specteros-os`**
3. Description: **"Privacy-First Debian-based Linux Distribution"**
4. Visibility: **Public** (recommended) or Private
5. **DO NOT** initialize with README, .gitignore, or license (we already have these)
6. Click **"Create repository"**

## Step 2: Link and Push

After creating the repo, run these commands:

```bash
# Replace YOUR_USERNAME with your GitHub username
# Replace REPO_NAME with your repo name (specteros or specteros-os)

git remote add origin https://github.com/YOUR_USERNAME/REPO_NAME.git

# Verify remote
git remote -v

# Push to GitHub
git push -u origin main
```

## Step 3: Verify

Visit your repository at:
```
https://github.com/YOUR_USERNAME/REPO_NAME
```

## Alternative: Using SSH

If you use SSH keys with GitHub:

```bash
# Add SSH remote instead
git remote add origin git@github.com:YOUR_USERNAME/REPO_NAME.git

# Push
git push -u origin main
```

## Step 4: Enable GitHub Actions (Optional)

1. Go to your repo **Settings** → **Actions** → **General**
2. Enable **"Allow all actions and reusable workflows"**
3. The CI workflows in `ci/workflows/` will now run on pushes

## Step 5: Add Topics

Add these topics to your repo for discoverability:
- `linux-distribution`
- `privacy`
- `debian`
- `security`
- `rust`
- `specteros`

---

## Quick Commands Reference

```bash
# Check current status
git status

# View commit history
git log --oneline

# Add new changes
git add -A
git commit -m "Description of changes"
git push

# Create a release tag
git tag -a v0.1.0 -m "SpecterOS v0.1.0"
git push origin --tags
```

---

**Your SpecterOS code is ready to push!** 🚀
