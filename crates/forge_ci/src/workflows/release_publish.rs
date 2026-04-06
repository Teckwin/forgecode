use gh_workflow::generate::Generate;
use gh_workflow::*;

use crate::jobs::ReleaseBuilderJob;

/// Generate release workflow
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
