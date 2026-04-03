use std::sync::Arc;

use anyhow::{Context, Ok};
use dashmap::DashMap;
use forge_app::{EnvironmentInfra, domain::SkillRepository};
use forge_domain::Skill;
use tokio::sync::RwLock;
// Import SkillRepositoryExt from forge_domain for dynamic operations
use forge_domain::SkillRepositoryExt;

/// SkillRegistryService manages runtime Skills in-memory. It lazily loads skills
/// from SkillRepository on first access and supports dynamic create/delete.
pub struct ForgeSkillRegistryService<R> {
    // Infrastructure dependency for loading skill definitions
    repository: Arc<R>,

    // Optional extended repository for dynamic create/delete operations
    // This allows backward compatibility when the extended traits are not available
    ext_repository: Option<Arc<dyn SkillRepositoryExt>>,

    // In-memory storage for skills keyed by skill name string
    // Lazily initialized on first access
    // Wrapped in RwLock to allow invalidation
    skills: RwLock<Option<DashMap<String, Skill>>>,
}

impl<R> ForgeSkillRegistryService<R> {
    /// Creates a new SkillRegistryService with the given repository
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository, ext_repository: None, skills: RwLock::new(None) }
    }

    /// Creates a new SkillRegistryService with both base and extended repository
    pub fn with_ext(repository: Arc<R>, ext_repository: Arc<dyn SkillRepositoryExt>) -> Self {
        Self {
            repository,
            ext_repository: Some(ext_repository),
            skills: RwLock::new(None),
        }
    }
}

impl<R: SkillRepository + EnvironmentInfra> ForgeSkillRegistryService<R> {
    /// Lazily initializes and returns the skills map
    /// Loads skills from repository on first call, subsequent calls return
    /// cached value
    async fn ensure_skills_loaded(&self) -> anyhow::Result<DashMap<String, Skill>> {
        // Check if already loaded
        {
            let skills_read = self.skills.read().await;
            if let Some(skills) = skills_read.as_ref() {
                return Ok(skills.clone());
            }
        }

        // Not loaded yet, acquire write lock and load
        let mut skills_write = self.skills.write().await;

        // Double-check in case another task loaded while we were waiting for write
        // lock
        if let Some(skills) = skills_write.as_ref() {
            return Ok(skills.clone());
        }

        // Load skills
        let skills_map = self.load_skills().await?;

        // Store and return
        *skills_write = Some(skills_map.clone());
        Ok(skills_map)
    }

    /// Load skills from repository
    async fn load_skills(&self) -> anyhow::Result<DashMap<String, Skill>> {
        // Load skill definitions from repository
        let skill_defs = self.repository.load_skills().await?;

        // Create the skills map
        let skills_map = DashMap::new();

        // Populate map with skill name as key
        for skill in skill_defs {
            skills_map.insert(skill.name.clone(), skill);
        }

        Ok(skills_map)
    }
}

#[async_trait::async_trait]
impl<R: SkillRepository + EnvironmentInfra> forge_app::SkillRegistry
    for ForgeSkillRegistryService<R>
{
    async fn get_skills(&self) -> anyhow::Result<Vec<Skill>> {
        let skills = self.ensure_skills_loaded().await?;
        Ok(skills.iter().map(|entry| entry.value().clone()).collect())
    }

    async fn get_skill(&self, skill_name: &str) -> anyhow::Result<Option<Skill>> {
        let skills = self.ensure_skills_loaded().await?;
        Ok(skills.get(skill_name).map(|v| v.value().clone()))
    }

    async fn reload_skills(&self) -> anyhow::Result<()> {
        *self.skills.write().await = None;

        self.ensure_skills_loaded().await?;
        Ok(())
    }

    async fn create_skill(&self, skill: Skill) -> anyhow::Result<()> {
        // Use the extended repository if available, otherwise return an error
        if let Some(ext) = &self.ext_repository {
            // Validate skill has required fields
            if skill.name.is_empty() {
                anyhow::bail!("Skill name cannot be empty");
            }
            if skill.command.is_empty() {
                anyhow::bail!("Skill command/prompt cannot be empty");
            }

            ext.create_skill(skill)
                .await
                .context("Failed to create skill")?;

            // Invalidate cache to reflect the new skill
            *self.skills.write().await = None;

            Ok(())
        } else {
            anyhow::bail!(
                "Dynamic skill creation is not available. \
                Please ensure the infrastructure supports SkillRepositoryExt."
            )
        }
    }

    async fn delete_skill(&self, skill_name: &str) -> anyhow::Result<()> {
        // Use the extended repository if available, otherwise return an error
        if let Some(ext) = &self.ext_repository {
            ext.delete_skill(skill_name)
                .await
                .context("Failed to delete skill")?;

            // Invalidate cache to reflect the deletion
            *self.skills.write().await = None;

            Ok(())
        } else {
            anyhow::bail!(
                "Dynamic skill deletion is not available. \
                Please ensure the infrastructure supports SkillRepositoryExt."
            )
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Note: Unit tests for ForgeSkillRegistryService require complex mock setup
    // due to the SkillRepository + EnvironmentInfra trait bounds.
    // The service is tested through integration tests in forge_app and forge_api.

    #[test]
    fn test_skill_registry_service_exists() {
        // Verify the struct can be instantiated (just check type exists)
        let _: ForgeSkillRegistryService<()> = ForgeSkillRegistryService::new(Arc::new(()));
    }
}
