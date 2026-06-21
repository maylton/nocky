# Publishing Nocky 0.1 Beta on GitHub

Recommended repository: `maylton/nocky`

## 1. Authenticate GitHub CLI over HTTPS

```bash
gh auth status
gh auth login
gh auth setup-git
```

Choose GitHub.com, HTTPS and browser authentication when prompted.

## 2. Initialize and review the repository

```bash
git init
git branch -M main
git add .
git status
git commit -m "Release Nocky 0.1 beta"
```

## 3. Create the public repository and push

```bash
gh repo create nocky \
  --public \
  --source=. \
  --remote=origin \
  --push \
  --description "A native GTK4/libadwaita music player for Linux"
```

If `origin` already exists, do not add it again. Use:

```bash
git remote set-url origin https://github.com/maylton/nocky.git
git push -u origin main
```

## 4. Create the beta tag

```bash
git tag -a v0.1.0-beta -m "Nocky 0.1 Beta"
git push origin v0.1.0-beta
```

## 5. Create the GitHub release

From inside the project directory:

```bash
gh release create v0.1.0-beta \
  ../nocky-0.1.0-beta-source.zip \
  ../nocky-0.1.0-beta-source.tar.gz \
  --title "Nocky 0.1 Beta" \
  --notes-file RELEASE_NOTES.md \
  --prerelease
```

## 6. Confirm after publishing

- The Actions tab shows a successful CI run.
- The release is marked as **Pre-release**.
- The source archives are attached.
- The README icon renders correctly.
- Issues are enabled for bug reports.
