# Force Delete — implementation and validation

Force Delete is an explicit maintenance tool in the sidebar and Optimize page. It is independent
of Cleaner: scanning junk or opening a page never invokes it. English is the source language;
Portuguese and Spanish have matching feature strings and placeholders.

## Workflow

1. Select local files with the native Windows multi-file dialog, select a folder, drop files/folders
   onto the page, or paste a full local path. Nothing destructive happens during selection.
2. Review the selection. The backend expands folders into a bounded snapshot, records file IDs,
   sizes and modification times, and queries Windows Restart Manager for blocking applications.
3. Confirm **Delete now**, **Terminate apps & delete**, or **Delete on restart**. The confirmation
   names the paths; the terminate action also names the affected applications. Cancel is focused
   by default. A review expires after ten minutes and its token cannot be replayed after execution.
4. Read the result. Deleted, scheduled, failed and terminated counts are separate. The first 100
   errors are displayed with their paths. Refresh the review before another attempt.

Limits: 100 selected roots, 5,000 total files/folders, 64 nested directory levels. Duplicate and
overlapping selections are collapsed. Inaccessible or unsupported descendants fail preparation,
so a partial inventory is not presented as a complete review.

## Native implementation

- `src-tauri/src/force_delete.rs` owns selection snapshots, native dialogs, Restart Manager,
  process checks, handle-based deletion and delayed-delete registration.
- `pick_force_delete`, `prepare_force_delete` and `execute_force_delete` run blocking work on
  Tauri's blocking runtime. Execution accepts a backend-generated token and a fixed action enum;
  it does not accept a frontend PID or a fresh path to delete.
- `src/force-delete.js` owns the selection/review/result state machine. `force-delete.css` uses
  the existing Skale Club palette, typography and surfaces. The feature is reachable from the
  sidebar and Optimize, including English, Portuguese and Spanish labels.
- Native file selection uses `GetOpenFileNameW`; folder selection uses `SHBrowseForFolderW` on
  an initialized STA worker, with the Xpecs window as owner. OS drag/drop uses Tauri's webview API.

## File boundaries

Only unambiguous absolute paths on local drive letters are accepted. Drive roots, UNC/device
paths, alternate data streams, wildcard paths, reserved DOS names and traversal components are
rejected. Windows/program trees, profile containers and the running executable directory are
protected. User files inside ordinary profile subfolders can be selected.

Every ancestor is opened without delete sharing and with directory-read access, keeping it pinned
through each mutation. Every path component and target is checked for reparse points. The physical
path and the open handle are checked against protected locations. Hard-linked files are refused,
including links added after review. This prevents clearing an attribute through an unreviewed alias.

Before immediate execution, all reviewed file identities are checked. Each deletion checks again
on the handle used for `SetFileInformationByHandle(FileDispositionInfo)`. The target handle denies
concurrent write/delete sharing. Files changed/replaced since review are preserved. A newly added
file is not added to the snapshot and prevents removal of its nonempty parent directory.

The read-only attribute may be cleared on the same handle, preserving other attributes; a failed
delete attempts to restore it. File ownership and ACLs are not changed. There is no kernel driver,
security bypass or promise that every locked/protected file can be removed.

## Process boundaries

Restart Manager receives reviewed file paths (directories are not registered). Its inventory is
advisory: a missing blocker does not prove a file is unlocked; query failures are displayed.
App termination is disabled if the inventory is unavailable or includes an ineligible process.

Only the applications shown in the review can be terminated. The backend refreshes the inventory,
compares PID **and creation time**, opens all process handles before terminating any process,
checks session membership, checks `IsProcessCritical`, and refuses services, Explorer, Windows
executables, Xpecs itself, PID 0/4 and unrecognized categories. PID reuse or a newly discovered
blocker requires a new review. Termination is explicit and may lose unsaved work; apps are not
automatically reopened. Partial termination failures report the completed count and do not proceed
to deletion.

## Deletion at restart

Administrator access is required. `MoveFileExW(path, NULL, MOVEFILE_DELAY_UNTIL_REBOOT)` registers
the reviewed paths with Windows, children first. A child registration failure prevents scheduling
its parent, and each success/failure is counted separately. This action never restarts Windows.

**Registration is not completed deletion.** It persists after Xpecs exits. Windows processes paths
at restart; a later replacement at a registered path may also be affected, and folders are removed
only if empty. The confirmation explains this path-based behavior. There is currently no in-app
cancellation of Windows' pending registrations. Permission failures, locks or later changes can
prevent removal; the tool does not guarantee success after restart.

## Automated verification

Run from the repository root:

```powershell
node --test scripts/force-delete.test.cjs
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run build
```

The Rust tests include disposable-file deletion, read-only handling, changed/replaced/new files,
locked-file preservation, path policy, hard links, root/nested/replaced junctions, pinned ancestors,
selection deduplication/expiration/replay, preflight of all files, Win32 structure layouts and
postorder/partial reboot registration with an injected scheduler.

An integration test starts a hidden disposable child process holding a fixture file. It verifies
the real Restart Manager inventory, refuses any unexpected blocker, rejects a creation-time
mismatch, terminates only that child, and verifies deletion of its fixture. Test cleanup reaps the
child. No existing user application is terminated.

The Node tests load the actual application scripts into a VM with DOM/IPC substitutes. They verify
selection without mutation, review, confirmation/cancel, duplicate-action suppression, stale-review
invalidation, picker cancellation, visible errors, partial results, HTML escaping, eligibility
controls and translation consistency. They do not validate native rendering or layout.

The normal suite **does not register real reboot deletions, restart the machine, open native picker
dialogs or launch the app on the desktop**. Delayed registration ordering/failure logic is tested
through the injected scheduler. Native picker interaction, UAC, visual layout and actual removal
after reboot remain manual Windows acceptance checks on disposable data.

## References

- [Restart Manager application types](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/ne-restartmanager-rm_app_type)
- [RmGetList](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmgetlist)
- [MoveFileExW and delayed operations](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
- [FILE_DISPOSITION_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_disposition_info)
- [Tauri webview drag/drop](https://v2.tauri.app/reference/javascript/api/namespacewebview/)
