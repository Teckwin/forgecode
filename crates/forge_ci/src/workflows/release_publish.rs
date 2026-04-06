use gh_workflow::generate::Generate;
use gh_workflow::*;

use crate::jobs::ReleaseBuilderJob;

/// Generate release workflow
///
/// ## IMPORTANT: Immutable Releases Constraint
///
/// GitHub enforces immutable releases on this repository. Once a release is
/// published, assets CANNOT be uploaded, modified, or deleted.
///
/// The actual build+upload happens in ci.yml via the draft release flow:
///   push to main → draft_release (creates draft) → build_release (builds + uploads to draft)
///
/// After ci.yml completes, the draft release has all 9 platform binaries attached.
/// A maintainer then manually publishes the draft via GitHub UI.
///
/// This release.yml workflow exists ONLY for the upstream npm/homebrew publish
/// jobs (which we don't use). We keep it to minimize diff with upstream.
///
/// ## DO NOT:
/// - Change the trigger from `published` to anything else
/// - Add asset upload steps (will fail with "Cannot upload assets to an immutable release")
/// - Remove the `release_id` from ReleaseBuilderJob (needed for upstream compat)
///
/// ## The correct release process:
/// 1. Merge PR to main → ci.yml builds binaries and uploads to draft release
/// 2. Go to GitHub Releases → find the draft → click "Publish release"
/// 3. That's it. No further build/upload needed.
pub fn release_publish() {
    let release_build_job = ReleaseBuilderJob::new("${{ github.event.release.tag_name }}")
        .release_id("${{ github.event.release.id }}");

    let workflow = Workflow::default()
        .name("Multi Channel Release")
        .on(Event {
            release: Some(Release { types: vec![ReleaseType::Published] }),
            ..Event::default()
        })
        .permissions(
            Permissions::default()
                .contents(Level::Write)
                .pull_requests(Level::Write),
        )
        .add_job("build_release", release_build_job.into_job());

    Generate::new(workflow)
        .name("release.yml")
        .generate()
        .unwrap();
}
