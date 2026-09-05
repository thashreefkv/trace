use std::path::PathBuf;

use project_manager_shared::{
    brain, gemini, keychain, mcp_log,
    models::{
        BrainCypherInput, BrainFeedbackInput, BrainLearningEventInput, BrainRetrieveInput,
        BrainTemplateInput, CaptureFilters, CreateCaptureInput, CreateConversationInput,
        CreateDeliverableInput, CreateInitiativeInput, CreateStakeholderInput, Deliverable,
        DeliverableFilters, DeliverableState, DeliverableType, MenuBarState, StakeholderBriefing,
        UpdateDeliverableInput, UpdateInitiativeInput, WorkGraphFilters,
    },
    repo,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, Json, ServerHandler,
};
use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;

use crate::tools::{
    BrainFeedbackToolInput, BrainLearningEventToolInput, BrainLearningSnapshotToolInput,
    CreateCaptureToolInput, CreateConversationToolInput, CreateDeliverableToolInput,
    CreateInitiativeToolInput, CreateStakeholderToolInput, DeleteDeliverableToolInput,
    DeleteInitiativeToolInput, DismissCaptureToolInput, EmptyToolInput, GetByIdToolInput,
    GetInitiativeStateToolInput, GetWorkContextGraphToolInput, ListCapturesToolInput,
    ListDeliverablesToolInput, ListInitiativesToolInput, ListStakeholderBriefingToolInput,
    PromoteCaptureToDeliverableToolInput, PromoteCaptureToInitiativeToolInput,
    QueryBrainCypherToolInput, RetrieveBrainContextToolInput, RunBrainTemplateToolInput,
    SearchDeliverablesToolInput, ToolDeliverableState, ToolDeliverableType, ToolResponse,
    UpdateDeliverableStateToolInput, UpdateDeliverableToolInput, UpdateInitiativeToolInput,
    DEFAULT_LIMIT, DEFAULT_SINCE_DAYS, MAX_LIMIT,
};

#[derive(Debug, Clone)]
pub struct TraceMcpServer {
    pool: SqlitePool,
    app_support_dir: PathBuf,
    brain_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TraceMcpServer {}

#[tool_router(router = tool_router)]
impl TraceMcpServer {
    pub fn new(pool: SqlitePool, app_support_dir: PathBuf) -> Self {
        let brain_path = app_support_dir.join(brain::BRAIN_FILE_NAME);
        Self {
            pool,
            app_support_dir,
            brain_path,
            tool_router: Self::tool_router(),
        }
    }

    pub fn tool_count(&self) -> usize {
        self.tool_router.list_all().len()
    }

    #[tool(
        name = "list_initiatives",
        description = "List initiatives, optionally filtered by status."
    )]
    async fn list_initiatives(
        &self,
        input: Parameters<ListInitiativesToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let mut initiatives = repo::list_initiatives(&self.pool).await?;
            if let Some(status) = input.status {
                let status = Into::<project_manager_shared::models::InitiativeStatus>::into(status);
                initiatives.retain(|initiative| initiative.status == status.as_str());
            }

            Ok(Json(ToolResponse::new(
                format!("Loaded {} initiative(s).", initiatives.len()),
                &initiatives,
            )?))
        }
        .await;

        self.log_tool("list_initiatives", &result, &[]);
        result
    }

    #[tool(name = "get_initiative", description = "Get one initiative by id.")]
    async fn get_initiative(
        &self,
        input: Parameters<GetByIdToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("initiative_id", input.id.clone())];
        let result = async {
            let initiative = repo::get_initiative(&self.pool, &input.id).await?;
            Ok(Json(ToolResponse::new(
                format!("Loaded initiative '{}'.", initiative.title),
                &initiative,
            )?))
        }
        .await;

        self.log_tool("get_initiative", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "create_initiative",
        description = "Create a deliberate initiative."
    )]
    async fn create_initiative(
        &self,
        input: Parameters<CreateInitiativeToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("title", input.title.clone())];
        let result = async {
            let initiative = repo::create_initiative(
                &self.pool,
                CreateInitiativeInput {
                    title: input.title,
                    framing: input.framing,
                    status: input.status.into(),
                    ..CreateInitiativeInput::default()
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Created initiative '{}'.", initiative.title),
                &initiative,
            )?))
        }
        .await;

        self.log_tool("create_initiative", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "update_initiative",
        description = "Update an initiative by id."
    )]
    async fn update_initiative(
        &self,
        input: Parameters<UpdateInitiativeToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("initiative_id", input.initiative_id.clone())];
        let result = async {
            let initiative = repo::update_initiative(
                &self.pool,
                &input.initiative_id,
                UpdateInitiativeInput {
                    title: input.title,
                    framing: input.framing,
                    status: input.status.into(),
                    ..UpdateInitiativeInput::default()
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Updated initiative '{}'.", initiative.title),
                &initiative,
            )?))
        }
        .await;

        self.log_tool("update_initiative", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "delete_initiative",
        description = "Delete an initiative only when confirm_title matches; linked deliverables also require confirm_orphan_deliverables."
    )]
    async fn delete_initiative(
        &self,
        input: Parameters<DeleteInitiativeToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("initiative_id", input.initiative_id.clone())];
        let result = async {
            let initiative = repo::get_initiative(&self.pool, &input.initiative_id).await?;
            if input.confirm_title != initiative.title {
                return Err(format!(
                    "confirm_title must exactly match '{}'.",
                    initiative.title
                ));
            }

            let linked = repo::list_deliverables_for_initiative(&self.pool, &initiative.id).await?;
            if !linked.is_empty() && !input.confirm_orphan_deliverables {
                return Err(format!(
                    "initiative has {} linked deliverable(s); set confirm_orphan_deliverables to true to remove the initiative link.",
                    linked.len()
                ));
            }

            repo::delete_initiative(&self.pool, &initiative.id).await?;
            let data = json!({
                "deleted_id": initiative.id,
                "deleted_title": initiative.title,
                "orphaned_deliverable_count": linked.len()
            });

            Ok(Json(ToolResponse::new("Deleted initiative.".to_string(), &data)?))
        }
        .await;

        self.log_tool("delete_initiative", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "create_deliverable",
        description = "Create a deliverable linked to exact initiative and stakeholder names."
    )]
    async fn create_deliverable(
        &self,
        input: Parameters<CreateDeliverableToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![
            ("title", input.title.clone()),
            ("initiatives", input.initiative_titles.join("|")),
        ];
        let result = async {
            let create_input = self
                .build_create_deliverable_input(
                    input.title,
                    input.deliverable_type,
                    input.state.unwrap_or(ToolDeliverableState::Drafting),
                    input.claim,
                    input.initiative_titles,
                    input.stakeholder_name,
                    input.stakeholder_names,
                    input.artifact_url,
                    input.conversation_url,
                )
                .await?;
            let deliverable = repo::create_deliverable(&self.pool, create_input).await?;

            Ok(Json(ToolResponse::new(
                format!("Created deliverable '{}'.", deliverable.title),
                &deliverable,
            )?))
        }
        .await;

        self.log_tool("create_deliverable", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "list_deliverables",
        description = "List deliverables with optional exact-name filters."
    )]
    async fn list_deliverables(
        &self,
        input: Parameters<ListDeliverablesToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let filters = self
                .resolve_filters(
                    input.initiative_title,
                    input.stakeholder_name,
                    input.deliverable_type,
                    input.state,
                )
                .await?;
            let deliverables = repo::list_deliverables(&self.pool, filters).await?;

            Ok(Json(ToolResponse::new(
                format!("Loaded {} deliverable(s).", deliverables.len()),
                &deliverables,
            )?))
        }
        .await;

        self.log_tool("list_deliverables", &result, &[]);
        result
    }

    #[tool(name = "get_deliverable", description = "Get one deliverable by id.")]
    async fn get_deliverable(
        &self,
        input: Parameters<GetByIdToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("deliverable_id", input.id.clone())];
        let result = async {
            let deliverable = repo::get_deliverable(&self.pool, &input.id).await?;
            Ok(Json(ToolResponse::new(
                format!("Loaded deliverable '{}'.", deliverable.title),
                &deliverable,
            )?))
        }
        .await;

        self.log_tool("get_deliverable", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "update_deliverable",
        description = "Partially update a deliverable by id."
    )]
    async fn update_deliverable(
        &self,
        input: Parameters<UpdateDeliverableToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("deliverable_id", input.deliverable_id.clone())];
        let result = async {
            let current = repo::get_deliverable(&self.pool, &input.deliverable_id).await?;
            let initiative_ids = match input.initiative_titles {
                Some(titles) => repo::resolve_initiative_titles(&self.pool, &titles).await?,
                None => current
                    .initiatives
                    .iter()
                    .map(|initiative| initiative.id.clone())
                    .collect(),
            };
            let stakeholder_ids = if input.clear_stakeholder.unwrap_or(false) {
                Vec::new()
            } else if let Some(names) = input.stakeholder_names {
                resolve_stakeholder_names(&self.pool, names).await?
            } else if let Some(name) = input.stakeholder_name {
                vec![repo::resolve_stakeholder_name(&self.pool, &name).await?]
            } else {
                current
                    .stakeholders
                    .iter()
                    .map(|stakeholder| stakeholder.id.clone())
                    .collect()
            };
            let stakeholder_id = stakeholder_ids.first().cloned();
            let conversation_id = if input.clear_conversation.unwrap_or(false) {
                None
            } else if let Some(url) = input.conversation_url {
                Some(
                    repo::create_or_get_conversation(&self.pool, &url, None, None, None)
                        .await?
                        .id,
                )
            } else {
                current.conversation_id.clone()
            };
            let artifact_url = if input.clear_artifact_url.unwrap_or(false) {
                None
            } else {
                input.artifact_url.or(current.artifact_url.clone())
            };

            let deliverable = repo::update_deliverable(
                &self.pool,
                &current.id,
                UpdateDeliverableInput {
                    title: input.title.unwrap_or(current.title),
                    deliverable_type: input
                        .deliverable_type
                        .map(Into::into)
                        .unwrap_or(deliverable_type_from_str(&current.deliverable_type)?),
                    state: input
                        .state
                        .map(Into::into)
                        .unwrap_or(deliverable_state_from_str(&current.state)?),
                    claim: input.claim.unwrap_or(current.claim),
                    artifact_url,
                    conversation_id,
                    stakeholder_id,
                    stakeholder_ids,
                    initiative_ids,
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Updated deliverable '{}'.", deliverable.title),
                &deliverable,
            )?))
        }
        .await;

        self.log_tool("update_deliverable", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "search_deliverables",
        description = "Search deliverable titles and claims with optional exact-name filters."
    )]
    async fn search_deliverables(
        &self,
        input: Parameters<SearchDeliverablesToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![
            ("query", input.query.clone()),
            ("limit", input.limit.unwrap_or(DEFAULT_LIMIT).to_string()),
        ];

        let result = async {
            let filters = self
                .resolve_filters(
                    input.initiative_title,
                    input.stakeholder_name,
                    input.deliverable_type,
                    input.state,
                )
                .await?;
            let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
            let deliverables =
                repo::search_deliverables(&self.pool, &input.query, filters, limit).await?;

            Ok(Json(ToolResponse::new(
                format!("Found {} deliverable(s).", deliverables.len()),
                &deliverables,
            )?))
        }
        .await;

        self.log_tool("search_deliverables", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "update_deliverable_state",
        description = "Update a deliverable state and preserve the shipped_at transition rule."
    )]
    async fn update_deliverable_state(
        &self,
        input: Parameters<UpdateDeliverableStateToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![
            ("deliverable_id", input.deliverable_id.clone()),
            ("state", input.state.as_str().to_string()),
        ];

        let result = async {
            let state = input.state.into();
            let deliverable =
                repo::update_deliverable_state(&self.pool, &input.deliverable_id, state).await?;

            Ok(Json(ToolResponse::new(
                format!(
                    "Updated deliverable '{}' to {}.",
                    deliverable.title, deliverable.state
                ),
                &deliverable,
            )?))
        }
        .await;

        self.log_tool("update_deliverable_state", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "delete_deliverable",
        description = "Delete a deliverable only when confirm_title matches."
    )]
    async fn delete_deliverable(
        &self,
        input: Parameters<DeleteDeliverableToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("deliverable_id", input.deliverable_id.clone())];
        let result = async {
            let deliverable = repo::get_deliverable(&self.pool, &input.deliverable_id).await?;
            if input.confirm_title != deliverable.title {
                return Err(format!(
                    "confirm_title must exactly match '{}'.",
                    deliverable.title
                ));
            }

            repo::delete_deliverable(&self.pool, &deliverable.id).await?;
            let data = json!({
                "deleted_id": deliverable.id,
                "deleted_title": deliverable.title
            });
            Ok(Json(ToolResponse::new(
                "Deleted deliverable.".to_string(),
                &data,
            )?))
        }
        .await;

        self.log_tool("delete_deliverable", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(name = "list_stakeholders", description = "List stakeholders.")]
    async fn list_stakeholders(
        &self,
        _input: Parameters<EmptyToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let result = async {
            let stakeholders = repo::list_stakeholders(&self.pool).await?;
            Ok(Json(ToolResponse::new(
                format!("Loaded {} stakeholder(s).", stakeholders.len()),
                &stakeholders,
            )?))
        }
        .await;

        self.log_tool("list_stakeholders", &result, &[]);
        result
    }

    #[tool(name = "create_stakeholder", description = "Create a stakeholder.")]
    async fn create_stakeholder(
        &self,
        input: Parameters<CreateStakeholderToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("name", input.name.clone())];
        let result = async {
            let stakeholder = repo::create_stakeholder(
                &self.pool,
                CreateStakeholderInput {
                    name: input.name,
                    role: String::new(),
                    notes: String::new(),
                    email: String::new(),
                },
            )
            .await?;
            Ok(Json(ToolResponse::new(
                format!("Created stakeholder '{}'.", stakeholder.name),
                &stakeholder,
            )?))
        }
        .await;

        self.log_tool("create_stakeholder", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "create_capture",
        description = "Create a capture in the inbox."
    )]
    async fn create_capture(
        &self,
        input: Parameters<CreateCaptureToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("kind", format!("{:?}", input.kind))];
        let result = async {
            let capture = repo::create_capture(
                &self.pool,
                CreateCaptureInput {
                    kind: input.kind.into(),
                    body: input.body,
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                "Created capture.".to_string(),
                &capture,
            )?))
        }
        .await;

        self.log_tool("create_capture", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "list_captures",
        description = "List captures with optional status and kind filters."
    )]
    async fn list_captures(
        &self,
        input: Parameters<ListCapturesToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let captures = repo::list_captures(
                &self.pool,
                CaptureFilters {
                    status: input.status.map(Into::into),
                    kind: input.kind.map(Into::into),
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Loaded {} capture(s).", captures.len()),
                &captures,
            )?))
        }
        .await;

        self.log_tool("list_captures", &result, &[]);
        result
    }

    #[tool(name = "get_capture", description = "Get one capture by id.")]
    async fn get_capture(
        &self,
        input: Parameters<GetByIdToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("capture_id", input.id.clone())];
        let result = async {
            let capture = repo::get_capture(&self.pool, &input.id).await?;
            Ok(Json(ToolResponse::new(
                "Loaded capture.".to_string(),
                &capture,
            )?))
        }
        .await;

        self.log_tool("get_capture", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "dismiss_capture",
        description = "Dismiss a capture only when confirm_body exactly matches its body."
    )]
    async fn dismiss_capture(
        &self,
        input: Parameters<DismissCaptureToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("capture_id", input.capture_id.clone())];
        let result = async {
            let capture = repo::get_capture(&self.pool, &input.capture_id).await?;
            if input.confirm_body != capture.body {
                return Err("confirm_body must exactly match the capture body.".to_string());
            }

            let capture = repo::dismiss_capture(&self.pool, &capture.id).await?;
            Ok(Json(ToolResponse::new(
                "Dismissed capture.".to_string(),
                &capture,
            )?))
        }
        .await;

        self.log_tool("dismiss_capture", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "promote_capture_to_deliverable",
        description = "Promote an inbox capture into a deliverable."
    )]
    async fn promote_capture_to_deliverable(
        &self,
        input: Parameters<PromoteCaptureToDeliverableToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("capture_id", input.capture_id.clone())];
        let result = async {
            let create_input = self
                .build_create_deliverable_input(
                    input.title,
                    input.deliverable_type,
                    input.state,
                    input.claim,
                    input.initiative_titles,
                    input.stakeholder_name,
                    input.stakeholder_names,
                    input.artifact_url,
                    input.conversation_url,
                )
                .await?;
            let deliverable =
                repo::promote_capture_to_deliverable(&self.pool, &input.capture_id, create_input)
                    .await?;

            Ok(Json(ToolResponse::new(
                format!("Promoted capture to deliverable '{}'.", deliverable.title),
                &deliverable,
            )?))
        }
        .await;

        self.log_tool("promote_capture_to_deliverable", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "promote_capture_to_initiative",
        description = "Promote an inbox thought capture into an initiative."
    )]
    async fn promote_capture_to_initiative(
        &self,
        input: Parameters<PromoteCaptureToInitiativeToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("capture_id", input.capture_id.clone())];
        let result = async {
            let initiative = repo::promote_capture_to_initiative(
                &self.pool,
                &input.capture_id,
                CreateInitiativeInput {
                    title: input.title,
                    framing: input.framing,
                    status: input.status.into(),
                    ..CreateInitiativeInput::default()
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Promoted capture to initiative '{}'.", initiative.title),
                &initiative,
            )?))
        }
        .await;

        self.log_tool("promote_capture_to_initiative", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "list_conversations",
        description = "List ingested conversations."
    )]
    async fn list_conversations(
        &self,
        _input: Parameters<EmptyToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let result = async {
            let conversations = repo::list_conversations(&self.pool).await?;
            Ok(Json(ToolResponse::new(
                format!("Loaded {} conversation(s).", conversations.len()),
                &conversations,
            )?))
        }
        .await;

        self.log_tool("list_conversations", &result, &[]);
        result
    }

    #[tool(name = "get_conversation", description = "Get one conversation by id.")]
    async fn get_conversation(
        &self,
        input: Parameters<GetByIdToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("conversation_id", input.id.clone())];
        let result = async {
            let conversation = repo::get_conversation(&self.pool, &input.id).await?;
            Ok(Json(ToolResponse::new(
                "Loaded conversation.".to_string(),
                &conversation,
            )?))
        }
        .await;

        self.log_tool("get_conversation", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "create_conversation",
        description = "Create or reuse a conversation row."
    )]
    async fn create_conversation(
        &self,
        input: Parameters<CreateConversationToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("chat_url", input.chat_url.clone())];
        let result = async {
            let conversation = repo::create_conversation(
                &self.pool,
                CreateConversationInput {
                    chat_url: input.chat_url,
                    title: input.title,
                    summary: input.summary,
                    occurred_at: input.occurred_at,
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                "Created or reused conversation.".to_string(),
                &conversation,
            )?))
        }
        .await;

        self.log_tool("create_conversation", &result, fields.as_slice());
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "get_initiative_state",
        description = "Get initiative framing, status, attached deliverables, and counts by deliverable state."
    )]
    async fn get_initiative_state(
        &self,
        input: Parameters<GetInitiativeStateToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("initiative", input.initiative_title.clone())];

        let result = async {
            let state =
                repo::get_initiative_state_by_title(&self.pool, &input.initiative_title).await?;

            Ok(Json(ToolResponse::new(
                format!("Loaded initiative state for '{}'.", state.initiative.title),
                &state,
            )?))
        }
        .await;

        self.log_tool("get_initiative_state", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "list_stakeholder_briefing",
        description = "Generate a stakeholder briefing from recent deliverables, using Gemini when configured and deterministic Markdown otherwise."
    )]
    async fn list_stakeholder_briefing(
        &self,
        input: Parameters<ListStakeholderBriefingToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let since_days = input.since_days.unwrap_or(DEFAULT_SINCE_DAYS).max(1);
        let fields = vec![
            ("stakeholder", input.stakeholder_name.clone()),
            ("since_days", since_days.to_string()),
        ];

        let result = async {
            let (stakeholder, deliverables) = repo::deliverables_for_stakeholder_since(
                &self.pool,
                &input.stakeholder_name,
                since_days,
            )
            .await?;

            let briefing = match keychain::get_gemini_api_key(&self.app_support_dir)? {
                Some(key) => match gemini::generate_stakeholder_briefing(
                    &self.pool,
                    &key,
                    &stakeholder,
                    &deliverables,
                    &[],
                    &[],
                    None,
                )
                .await
                {
                    Ok(briefing) => briefing,
                    Err(_) => {
                        gemini::fallback_stakeholder_briefing(stakeholder, deliverables)
                    }
                },
                None => {
                    gemini::fallback_stakeholder_briefing(stakeholder, deliverables)
                }
            };

            Ok(Json(ToolResponse::new(
                briefing_message(&briefing),
                &briefing,
            )?))
        }
        .await;

        self.log_tool("list_stakeholder_briefing", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "get_work_context_graph",
        description = "Return the relational Trace work graph."
    )]
    async fn get_work_context_graph(
        &self,
        input: Parameters<GetWorkContextGraphToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let graph = brain::get_work_context_graph(
                &self.pool,
                &self.brain_path,
                WorkGraphFilters {
                    include_dismissed_captures: input.include_dismissed_captures.unwrap_or(false),
                    include_killed_deliverables: input.include_killed_deliverables.unwrap_or(false),
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!(
                    "Loaded work graph with {} node(s) and {} edge(s).",
                    graph.nodes.len(),
                    graph.edges.len()
                ),
                &graph,
            )?))
        }
        .await;

        self.log_tool("get_work_context_graph", &result, &[]);
        result
    }

    #[tool(
        name = "retrieve_brain_context",
        description = "Retrieve compact, ranked Kuzu brain graph context for a question, including nearby entities and relation summaries."
    )]
    async fn retrieve_brain_context(
        &self,
        input: Parameters<RetrieveBrainContextToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![
            ("query", input.query.clone()),
            (
                "focus_entity_id",
                input.focus_entity_id.clone().unwrap_or_default(),
            ),
        ];
        let result = async {
            let context = brain::retrieve_brain_context(
                &self.pool,
                &self.brain_path,
                BrainRetrieveInput {
                    query: input.query,
                    focus_entity_id: input.focus_entity_id,
                    max_hops: input.max_hops,
                    limit: input.limit,
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!(
                    "Retrieved brain context with {} node(s) and {} relation(s).",
                    context.graph.nodes.len(),
                    context.graph.edges.len()
                ),
                &context,
            )?))
        }
        .await;

        self.log_tool("retrieve_brain_context", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "query_brain_cypher",
        description = "Run a bounded read-only Cypher query against the local Kuzu brain graph. Write keywords are rejected."
    )]
    async fn query_brain_cypher(
        &self,
        input: Parameters<QueryBrainCypherToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let fields = vec![("query", input.query.clone())];
        let result = async {
            if !self.brain_path.exists() {
                let _ = brain::rebuild_brain(&self.pool, &self.brain_path).await;
            }
            let rows = brain::query_brain_cypher(
                &self.brain_path,
                BrainCypherInput {
                    query: input.query,
                    params: input.params,
                    limit: input.limit,
                },
            )
            .await?;

            Ok(Json(ToolResponse::new(
                format!("Brain Cypher returned {} row(s).", rows.rows.len()),
                &rows,
            )?))
        }
        .await;

        self.log_tool("query_brain_cypher", &result, fields.as_slice());
        result
    }

    #[tool(
        name = "run_brain_template",
        description = "Run a deterministic second-brain graph template such as focus_today, blocked_work, email_followups, stale_work, or stakeholder_context."
    )]
    async fn run_brain_template(
        &self,
        input: Parameters<RunBrainTemplateToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let result = brain::run_brain_template(
                &self.pool,
                &self.brain_path,
                BrainTemplateInput {
                    template: input.template.into(),
                    focus_entity_id: input.focus_entity_id,
                    limit: input.limit,
                },
            )
            .await?;
            Ok(Json(ToolResponse::new(
                format!(
                    "Brain template '{}' returned {} row(s).",
                    result.template,
                    result.rows.len()
                ),
                &result,
            )?))
        }
        .await;

        self.log_tool("run_brain_template", &result, &[]);
        result
    }

    #[tool(
        name = "get_daily_brain_brief",
        description = "Generate the daily second-brain brief: focus today, blockers, follow-up emails, stale work, and inferred links to review."
    )]
    async fn get_daily_brain_brief(
        &self,
        _input: Parameters<EmptyToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let result = async {
            let brief = brain::get_daily_brain_brief(&self.pool, &self.brain_path).await?;
            Ok(Json(ToolResponse::new(
                "Generated daily brain brief.".to_string(),
                &brief,
            )?))
        }
        .await;

        self.log_tool("get_daily_brain_brief", &result, &[]);
        result
    }

    #[tool(
        name = "record_brain_feedback",
        description = "Record whether a brain answer was useful, wrong, or ignored, optionally with corrected inference relationships."
    )]
    async fn record_brain_feedback(
        &self,
        input: Parameters<BrainFeedbackToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            brain::record_brain_feedback(
                &self.pool,
                BrainFeedbackInput {
                    question: input.question,
                    template: input.template,
                    feedback: input.feedback,
                    corrected: input.corrected,
                },
            )
            .await?;
            Ok(Json(ToolResponse::new(
                "Recorded brain feedback.".to_string(),
                &json!({ "ok": true }),
            )?))
        }
        .await;

        self.log_tool("record_brain_feedback", &result, &[]);
        self.refresh_brain_after_write(&result).await;
        result
    }

    #[tool(
        name = "record_brain_learning_event",
        description = "Record a user interaction event for the local second-brain contextual bandit, such as shown, clicked, useful, wrong, ignored, completed_after_seen, accepted_inference, or dismissed."
    )]
    async fn record_brain_learning_event(
        &self,
        input: Parameters<BrainLearningEventToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let event = brain::record_brain_learning_event(
                &self.pool,
                BrainLearningEventInput {
                    template: input.template,
                    item_id: input.item_id,
                    item_kind: input.item_kind,
                    event_type: input.event_type,
                    reward: input.reward,
                    context: input.context,
                },
            )
            .await?;
            Ok(Json(ToolResponse::new(
                "Recorded brain learning event.".to_string(),
                &event,
            )?))
        }
        .await;

        self.log_tool("record_brain_learning_event", &result, &[]);
        result
    }

    #[tool(
        name = "get_brain_learning_snapshot",
        description = "Inspect the local second-brain learning policy state, recent feedback events, and top learned item scores."
    )]
    async fn get_brain_learning_snapshot(
        &self,
        input: Parameters<BrainLearningSnapshotToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let input = input.0;
        let result = async {
            let snapshot =
                brain::get_brain_learning_snapshot(&self.pool, input.template, input.limit).await?;
            Ok(Json(ToolResponse::new(
                "Loaded brain learning snapshot.".to_string(),
                &snapshot,
            )?))
        }
        .await;

        self.log_tool("get_brain_learning_snapshot", &result, &[]);
        result
    }

    #[tool(
        name = "get_current_focus",
        description = "Return the current menu-bar focus deliverable and shipped-this-week count."
    )]
    async fn get_current_focus(
        &self,
        _input: Parameters<EmptyToolInput>,
    ) -> Result<Json<ToolResponse>, String> {
        let result = async {
            let menu_bar_state = repo::menu_bar_state(&self.pool).await?;
            let active_deliverable = match &menu_bar_state.active_deliverable_id {
                Some(id) => Some(repo::get_deliverable(&self.pool, id).await?),
                None => None,
            };
            let focus = CurrentFocus {
                menu_bar_state,
                active_deliverable,
            };

            Ok(Json(ToolResponse::new(
                "Loaded current focus.".to_string(),
                &focus,
            )?))
        }
        .await;

        self.log_tool("get_current_focus", &result, &[]);
        result
    }

    async fn build_create_deliverable_input(
        &self,
        title: String,
        deliverable_type: ToolDeliverableType,
        state: ToolDeliverableState,
        claim: String,
        initiative_titles: Vec<String>,
        stakeholder_name: Option<String>,
        stakeholder_names: Option<Vec<String>>,
        artifact_url: Option<String>,
        conversation_url: Option<String>,
    ) -> Result<CreateDeliverableInput, String> {
        let initiative_ids =
            repo::resolve_initiative_titles(&self.pool, &initiative_titles).await?;
        let stakeholder_ids = match stakeholder_names {
            Some(names) => resolve_stakeholder_names(&self.pool, names).await?,
            None => match clean_optional(stakeholder_name) {
                Some(name) => vec![repo::resolve_stakeholder_name(&self.pool, &name).await?],
                None => Vec::new(),
            },
        };
        let stakeholder_id = stakeholder_ids.first().cloned();
        let conversation_id = match clean_optional(conversation_url) {
            Some(url) => Some(
                repo::create_or_get_conversation(&self.pool, &url, None, None, None)
                    .await?
                    .id,
            ),
            None => None,
        };

        Ok(CreateDeliverableInput {
            title,
            deliverable_type: deliverable_type.into(),
            state: state.into(),
            claim,
            artifact_url: clean_optional(artifact_url),
            conversation_id,
            stakeholder_id,
            stakeholder_ids,
            initiative_ids,
        })
    }

    async fn resolve_filters(
        &self,
        initiative_title: Option<String>,
        stakeholder_name: Option<String>,
        deliverable_type: Option<ToolDeliverableType>,
        state: Option<ToolDeliverableState>,
    ) -> Result<DeliverableFilters, String> {
        let initiative_id = match clean_optional(initiative_title) {
            Some(title) => Some(repo::resolve_initiative_title(&self.pool, &title).await?),
            None => None,
        };
        let stakeholder_id = match clean_optional(stakeholder_name) {
            Some(name) => Some(repo::resolve_stakeholder_name(&self.pool, &name).await?),
            None => None,
        };

        Ok(DeliverableFilters {
            initiative_id,
            stakeholder_id,
            deliverable_type: deliverable_type.map(Into::into),
            state: state.map(Into::into),
            state_in: None,
            priority: None,
        })
    }

    async fn refresh_brain_after_write(&self, result: &Result<Json<ToolResponse>, String>) {
        if result.is_ok() {
            // Fire-and-forget with coalescing — see `brain::request_rebuild`.
            // Awaiting inline added 50–100s per write tool when N concurrent
            // writes queued behind the rebuild mutex.
            brain::request_rebuild(self.pool.clone(), self.brain_path.clone());
        }
    }

    fn log_tool(
        &self,
        tool_name: &str,
        result: &Result<Json<ToolResponse>, String>,
        fields: &[(&str, String)],
    ) {
        let mut summary = fields.to_vec();
        if let Err(error) = result {
            summary.push(("error", error.clone()));
        }

        let _ = mcp_log::append_tool_log(
            &self.app_support_dir,
            tool_name,
            result.is_ok(),
            summary.as_slice(),
        );

        // Also write to the structured audit table so the Settings panel can show it.
        let pool = self.pool.clone();
        let tool = tool_name.to_string();
        let args_obj = serde_json::Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.clone())))
                .collect(),
        );
        let args_json = serde_json::to_string(&args_obj).unwrap_or_else(|_| "{}".to_string());
        let ok = result.is_ok();
        let error_str = match result {
            Ok(_) => None,
            Err(e) => Some(e.clone()),
        };
        let result_summary = match result {
            Ok(Json(response)) => Some(response.message.clone()),
            Err(_) => None,
        };
        project_manager_shared::runtime::spawn(async move {
            project_manager_shared::tool_audit::record(
                &pool,
                project_manager_shared::tool_audit::RecordInput {
                    source: "mcp",
                    run_id: None,
                    call_id: None,
                    tool: &tool,
                    args_json: &args_json,
                    result_summary: result_summary.as_deref(),
                    result_json: None,
                    ok,
                    error: error_str.as_deref(),
                    latency_ms: 0,
                },
            )
            .await;
        });
    }
}

#[derive(Debug, Serialize)]
struct CurrentFocus {
    menu_bar_state: MenuBarState,
    active_deliverable: Option<Deliverable>,
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn resolve_stakeholder_names(
    pool: &SqlitePool,
    names: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for name in names {
        if let Some(name) = clean_optional(Some(name)) {
            let id = repo::resolve_stakeholder_name(pool, &name).await?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

fn deliverable_type_from_str(value: &str) -> Result<DeliverableType, String> {
    match value {
        "deck" => Ok(DeliverableType::Deck),
        "design_doc" => Ok(DeliverableType::DesignDoc),
        "prototype" => Ok(DeliverableType::Prototype),
        "analysis" => Ok(DeliverableType::Analysis),
        "framework" => Ok(DeliverableType::Framework),
        "pitch" => Ok(DeliverableType::Pitch),
        "research" => Ok(DeliverableType::Research),
        "code" => Ok(DeliverableType::Code),
        "email" => Ok(DeliverableType::Email),
        "meeting_prep" => Ok(DeliverableType::MeetingPrep),
        "other" => Ok(DeliverableType::Other),
        other => Err(format!("unknown deliverable type in database: {other}")),
    }
}

fn deliverable_state_from_str(value: &str) -> Result<DeliverableState, String> {
    match value {
        "drafting" => Ok(DeliverableState::Drafting),
        "in_review" => Ok(DeliverableState::InReview),
        "shipped" => Ok(DeliverableState::Shipped),
        "killed" => Ok(DeliverableState::Killed),
        other => Err(format!("unknown deliverable state in database: {other}")),
    }
}

fn briefing_message(briefing: &StakeholderBriefing) -> String {
    format!(
        "Generated {} briefing for '{}': {}",
        briefing.generated_with, briefing.stakeholder.name, briefing.sections.tldr
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_manager_shared::db;
    use serde_json::json;
    use tempfile::tempdir;

    async fn test_server() -> TraceMcpServer {
        let pool = db::connect_memory().await.expect("memory db");
        repo::create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Enterprise Search".to_string(),
                framing: "Reduce retrieval friction.".to_string(),
                status: project_manager_shared::models::InitiativeStatus::Live,
                ..CreateInitiativeInput::default()
            },
        )
        .await
        .expect("initiative");
        repo::create_stakeholder(
            &pool,
            CreateStakeholderInput {
                name: "CEO".to_string(),
                role: String::new(),
                notes: String::new(),
                email: String::new(),
            },
        )
        .await
        .expect("stakeholder");

        TraceMcpServer::new(pool, tempdir().expect("tempdir").keep())
    }

    #[tokio::test]
    async fn exposes_expanded_tool_surface() {
        let pool = db::connect_memory().await.expect("memory db");
        let server = TraceMcpServer::new(pool, PathBuf::from("/tmp"));
        assert_eq!(server.tool_count(), 34);
    }

    #[tokio::test]
    async fn create_deliverable_reuses_conversation_rows() {
        let server = test_server().await;
        let input = CreateDeliverableToolInput {
            title: "Search plan".to_string(),
            deliverable_type: ToolDeliverableType::DesignDoc,
            claim: "We can ship search in slices.".to_string(),
            initiative_titles: vec!["Enterprise Search".to_string()],
            stakeholder_name: Some("CEO".to_string()),
            stakeholder_names: None,
            artifact_url: None,
            conversation_url: Some("https://claude.ai/chat/abc123".to_string()),
            state: None,
        };

        let result = server
            .create_deliverable(Parameters(input))
            .await
            .expect("created");
        assert!(result.0.message.contains("Created"));
        assert_eq!(
            result.0.data["conversation_url"],
            json!("https://claude.ai/chat/abc123")
        );
    }

    #[tokio::test]
    async fn search_rejects_non_exact_initiative_name() {
        let server = test_server().await;
        let input = SearchDeliverablesToolInput {
            query: "search".to_string(),
            initiative_title: Some("Enterprise".to_string()),
            stakeholder_name: None,
            deliverable_type: None,
            state: None,
            limit: None,
        };

        let error = match server.search_deliverables(Parameters(input)).await {
            Ok(_) => panic!("strict resolution should fail"),
            Err(error) => error,
        };
        assert!(error.contains("Valid initiatives"));
    }

    #[tokio::test]
    async fn delete_deliverable_requires_matching_confirmation() {
        let server = test_server().await;
        let created = server
            .create_deliverable(Parameters(CreateDeliverableToolInput {
                title: "Delete me".to_string(),
                deliverable_type: ToolDeliverableType::Analysis,
                claim: "This validates confirmation.".to_string(),
                initiative_titles: vec!["Enterprise Search".to_string()],
                stakeholder_name: None,
                stakeholder_names: None,
                artifact_url: None,
                conversation_url: None,
                state: None,
            }))
            .await
            .expect("created");
        let deliverable_id = created.0.data["id"].as_str().expect("id").to_string();

        let error = match server
            .delete_deliverable(Parameters(DeleteDeliverableToolInput {
                deliverable_id: deliverable_id.clone(),
                confirm_title: "Wrong".to_string(),
            }))
            .await
        {
            Ok(_) => panic!("confirmation should fail"),
            Err(error) => error,
        };
        assert!(error.contains("confirm_title"));

        server
            .delete_deliverable(Parameters(DeleteDeliverableToolInput {
                deliverable_id,
                confirm_title: "Delete me".to_string(),
            }))
            .await
            .expect("deleted");
    }

    #[tokio::test]
    async fn dismiss_capture_requires_body_confirmation() {
        let server = test_server().await;
        let capture = server
            .create_capture(Parameters(CreateCaptureToolInput {
                kind: crate::tools::ToolCaptureKind::Thought,
                body: "Captured body".to_string(),
            }))
            .await
            .expect("capture");
        let capture_id = capture.0.data["id"].as_str().expect("id").to_string();

        let error = match server
            .dismiss_capture(Parameters(DismissCaptureToolInput {
                capture_id: capture_id.clone(),
                confirm_body: "captured body".to_string(),
            }))
            .await
        {
            Ok(_) => panic!("confirmation should fail"),
            Err(error) => error,
        };
        assert!(error.contains("confirm_body"));

        server
            .dismiss_capture(Parameters(DismissCaptureToolInput {
                capture_id,
                confirm_body: "Captured body".to_string(),
            }))
            .await
            .expect("dismissed");
    }
}
