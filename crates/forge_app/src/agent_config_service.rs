use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use merge::Merge;
use forge_domain::{
    Agent, AgentDefinition, AgentId, Environment, ModelId, ProviderId,
};

/// AgentConfigService - 配置服务封装
/// 负责读取和合并Agent配置，支持多层级配置优先级：
/// 项目配置 > 全局配置 > 环境变量 > 代码默认
#[async_trait]
pub trait AgentConfigService: Send + Sync {
    /// 获取指定Agent的合并配置
    async fn get_agent_config(&self, agent_id: &AgentId) -> Result<Option<Agent>>;

    /// 获取所有Agent的配置
    async fn get_all_agent_configs(&self) -> Result<Vec<Agent>>;

    /// 重新加载配置
    async fn reload_configs(&self) -> Result<()>;
}

/// 配置合并器 - 负责实现配置优先级merge逻辑
/// 优先级: Agent定义 > 项目配置 > 全局配置 > 默认配置
pub struct AgentConfigMerger {
    environment: Arc<Environment>,
}

impl AgentConfigMerger {
    pub fn new(environment: Environment) -> Self {
        Self {
            environment: Arc::new(environment),
        }
    }

    /// 合并AgentDefinition和全局配置
    /// 使用提供的默认provider和model
    /// 优先级: Agent定义 > 全局配置 > 环境默认
    pub fn merge_with_defaults(
        &self,
        agent_def: AgentDefinition,
        default_provider: ProviderId,
        default_model: ModelId,
    ) -> Agent {
        // 从AgentDefinition创建基础Agent，使用传入的默认值
        let mut agent = Agent::from_agent_def(agent_def, default_provider, default_model);

        // 应用环境级配置（如果Agent没有设置的话）
        // 优先级: Agent定义 > 环境配置 > 默认值

        // 推理参数
        if agent.temperature.is_none() {
            agent.temperature = self.environment.temperature;
        }
        if agent.top_p.is_none() {
            agent.top_p = self.environment.top_p;
        }
        if agent.top_k.is_none() {
            agent.top_k = self.environment.top_k;
        }
        if agent.max_tokens.is_none() {
            agent.max_tokens = self.environment.max_tokens;
        }

        // 行为控制参数
        if agent.max_tool_failure_per_turn.is_none() {
            agent.max_tool_failure_per_turn = self.environment.max_tool_failure_per_turn;
        }
        if agent.max_requests_per_turn.is_none() {
            agent.max_requests_per_turn = self.environment.max_requests_per_turn;
        }

        // 重入检测参数
        if agent.reenter_limit.is_none() {
            agent.reenter_limit = self.environment.reenter_limit;
        }
        if agent.reenter_window_secs.is_none() {
            agent.reenter_window_secs = self.environment.reenter_window_secs;
        }

        // 上下文压缩配置 - 合并workflow compact到agent compact
        if let Some(ref workflow_compact) = self.environment.compact {
            let mut merged_compact = workflow_compact.clone();
            merged_compact.merge(agent.compact.clone());
            agent.compact = merged_compact;
        }

        // 注意: api_key 和 base_url 在AgentDefinition中已经设置
        // 这里不需要额外覆盖，因为Agent级别的配置已经优先于全局配置

        agent
    }

    /// 合并AgentDefinition和全局配置
    /// 从Environment中获取默认provider和model
    /// 优先级: Agent定义 > 全局配置 > 环境默认
    pub fn merge(&self, agent_def: AgentDefinition) -> Agent {
        // 从全局配置获取默认provider和model
        let default_provider = self
            .environment
            .session
            .as_ref()
            .and_then(|s| s.provider_id.as_ref())
            .map(|p| ProviderId::from(p.to_string()))
            .unwrap_or(ProviderId::FORGE);

        let default_model = self
            .environment
            .session
            .as_ref()
            .and_then(|s| s.model_id.as_ref())
            .map(|m| ModelId::new(m.to_string()))
            .unwrap_or_else(|| ModelId::new("claude-sonnet-4-6"));

        self.merge_with_defaults(agent_def, default_provider, default_model)
    }
}