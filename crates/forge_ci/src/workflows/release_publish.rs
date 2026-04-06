use gh_workflow::generate::Generate;
use gh_workflow::*;

use crate::jobs::ReleaseBuilderJob;

/// Generate release workflow
///
/// Uses `release: [created]` trigger so draft releases can be used.
/// This is required for GitHub's immutable releases feature — published
/// releases cannot have assets uploaded to them. The workflow:
/// 1. Triggers on draft release creation
/// 2. Builds and uploads binaries to the draft release
/// 3. Publishes the draft release after all uploads complete
pub fn release_publish() {
    let release_build_job = ReleaseBuilderJob::new("${{ github.event.release.tag_name }}")
        .release_id("${{ github.event.release.id }}");

    // Publish the draft release after all builds complete
    let publish_release_job = Job::new("publish-release")
        .add_needs("build_release")
        .runs_on("ubuntu-latest")
        .cond(Expression::new("github.event.release.draft == true"))
        .permissions(Permissions::default().contents(Level::Write))
        .add_step(
            Step::new("Publish Release")
                .run("gh release edit ${{ github.event.release.tag_name }} --draft=false --repo ${{ github.repository }}")
                .add_env(("GH_TOKEN", "${{ secrets.GITHUB_TOKEN }}")),
        );

    let workflow = Workflow::default()
        .name("Multi Channel Release")
        .on(Event {
            release: Some(Release { types: vec![ReleaseType::Created] }),
            workflow_dispatch: Some(WorkflowDispatch::default()),
            ..Event::default()
        })
        .permissions(
            Permissions::default()
                .contents(Level::Write)
                .pull_requests(Level::Write),
        )
        .add_job("build_release", release_build_job.into_job())
        .add_job("publish_release", publish_release_job);

    Generate::new(workflow)
        .name("release.yml")
        .generate()
        .unwrap();
}
