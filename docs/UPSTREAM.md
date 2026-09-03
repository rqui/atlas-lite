# Upstream Strategy

## Relationship

Atlas Lite is intended to be a GitHub fork of:

```text
aimindseye/rustmix-wave
```

under:

```text
rqui/atlas-lite
```

Expected remotes:

```text
origin   rqui/atlas-lite
upstream aimindseye/rustmix-wave
```

Verify with:

```bash
git remote -v
```

## Base pin

The planning audit used Rustmix Wave `main` at:

```text
6feeeb4f5941bf9b899033f713dcc5f2987e8bad
```

Do not assume this is still the latest upstream commit when implementation begins. Before M0 starts:

```bash
git fetch upstream
git log --oneline --decorate -n 10 upstream/main
```

Record the exact upstream base in the implementation ledger and PR description.

## Sync policy

Never make Atlas-specific commits on `upstream/main`.

Typical review flow:

```bash
git fetch upstream
git log --oneline --left-right --graph origin/main...upstream/main
```

For a deliberate upstream sync:

1. create an isolated sync branch/worktree;
2. inspect upstream changes;
3. merge or rebase according to the fork's established policy;
4. run host validation;
5. run target build when platform code changed;
6. hardware-test display/input/power/audio/network changes as applicable;
7. open a Draft PR;
8. never silently auto-merge upstream.

## Minimize fork drift

Prefer Atlas-specific additions in:

```text
src/atlas/
src/app/screens/atlas_*
```

and small routing/product-shell changes over broad edits to hardware drivers.

Platform modules should be modified only when:

- the upstream abstraction blocks an Atlas requirement;
- an actual hardware defect is verified;
- a measured resource/power problem requires a platform change.

When a platform file is modified, document why in the PR.

## Feature removal policy

During early milestones, hide or disable Rustmix's unrelated applications from the Atlas navigation shell rather than deleting their source immediately.

Reasons:

- lower regression risk;
- easier upstream rebases;
- easier comparison against known-good hardware behavior;
- easier recovery if a shared subsystem was accidentally coupled to an old app.

After Atlas Lite is stable, dead product modules may be pruned in a dedicated cleanup milestone with size/build evidence.

## License and attribution

Keep the upstream MIT license and required notices.

Atlas Lite documentation must state that the firmware is based on Rustmix Wave and link to the upstream project.

Do not imply that Atlas Lite is an official Rustmix Wave release.
