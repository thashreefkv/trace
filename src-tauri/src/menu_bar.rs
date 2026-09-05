use tauri::{
    menu::{Menu, MenuBuilder, MenuEvent, MenuItemBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    App, AppHandle, Manager, State, Wry,
};

use crate::{commands::windows::show_main_window_inner, db::AppState, models::MenuBarState};

const TRAY_ID: &str = "project-manager-tray";
const MENU_OPEN_ACTIVE: &str = "open-active-deliverable";
const MENU_MARK_SHIPPED: &str = "mark-active-shipped";
const MENU_OPEN_APP: &str = "open-app";
const MENU_QUIT: &str = "quit";

pub(crate) struct MenuBarTray {
    #[allow(dead_code)]
    tray: TrayIcon<Wry>,
}

#[tauri::command]
pub async fn get_menu_bar_state(state: State<'_, AppState>) -> Result<MenuBarState, String> {
    project_manager_shared::repo::menu_bar_state(&state.pool).await
}

#[tauri::command]
pub async fn refresh_menu_bar(app: AppHandle) -> Result<(), String> {
    refresh_menu_bar_inner(&app).await
}

pub(crate) fn setup_menu_bar(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let pool = app.state::<AppState>().pool.clone();
    let state = tauri::async_runtime::block_on(project_manager_shared::repo::menu_bar_state(&pool))
        .map_err(std::io::Error::other)?;
    let tooltip = tooltip_for_state(&state);
    let menu = build_menu(app.handle(), &state)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .show_menu_on_left_click(false)
        .tooltip(tooltip)
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event))
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let _ = crate::commands::windows::toggle_tray_window(tray.app_handle());
            }
        });

    let icon = tauri::include_image!("icons/trayTemplate.png");
    builder = builder.icon(icon).icon_as_template(true);

    let tray = builder.build(app)?;
    app.manage(MenuBarTray { tray });
    Ok(())
}

pub(crate) async fn refresh_menu_bar_inner(app: &AppHandle) -> Result<(), String> {
    let pool = app.state::<AppState>().pool.clone();
    let state = project_manager_shared::repo::menu_bar_state(&pool).await?;
    let menu = build_menu(app, &state).map_err(|error| error.to_string())?;

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_title(Some(&state.tray_title))
            .map_err(|error| error.to_string())?;
        tray.set_tooltip(Some(tooltip_for_state(&state)))
            .map_err(|error| error.to_string())?;
        tray.set_menu(Some(menu))
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn build_menu(app: &AppHandle, state: &MenuBarState) -> tauri::Result<Menu<Wry>> {
    let active_label = state
        .active_deliverable_title
        .as_deref()
        .map(|title| {
            format!(
                "Active: {}",
                project_manager_shared::repo::truncate_tray_title(title)
            )
        })
        .unwrap_or_else(|| "Active: none".to_string());
    let active_item = MenuItemBuilder::with_id("active-deliverable-title", active_label)
        .enabled(false)
        .build(app)?;
    let open_active = MenuItemBuilder::with_id(MENU_OPEN_ACTIVE, "Open active deliverable")
        .enabled(state.active_deliverable_id.is_some())
        .build(app)?;
    let mark_shipped = MenuItemBuilder::with_id(MENU_MARK_SHIPPED, "Mark active finished")
        .enabled(state.active_deliverable_id.is_some())
        .build(app)?;
    let shipped_count = MenuItemBuilder::with_id(
        "shipped-this-week",
        format!("Finished this week: {}", state.shipped_this_week),
    )
    .enabled(false)
    .build(app)?;
    let open_app = MenuItemBuilder::with_id(MENU_OPEN_APP, "Open app").build(app)?;
    let quit = MenuItemBuilder::with_id(MENU_QUIT, "Quit Trace").build(app)?;

    MenuBuilder::new(app)
        .item(&active_item)
        .separator()
        .item(&open_active)
        .item(&mark_shipped)
        .separator()
        .item(&shipped_count)
        .separator()
        .item(&open_app)
        .separator()
        .item(&quit)
        .build()
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref().to_string();
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        match id.as_str() {
            MENU_OPEN_ACTIVE => {
                let _ = open_active_deliverable(&app).await;
            }
            MENU_MARK_SHIPPED => {
                let _ = mark_active_deliverable_shipped(&app).await;
            }
            MENU_OPEN_APP => {
                let _ = show_main_window_inner(&app, Some("/deliverables"));
            }
            MENU_QUIT => {
                app.exit(0);
            }
            _ => {}
        }
    });
}

async fn open_active_deliverable(app: &AppHandle) -> Result<(), String> {
    let pool = app.state::<AppState>().pool.clone();
    let state = project_manager_shared::repo::menu_bar_state(&pool).await?;
    let route = state
        .active_deliverable_id
        .map(|id| format!("/deliverables/{id}"))
        .unwrap_or_else(|| "/deliverables".to_string());

    show_main_window_inner(app, Some(&route))
}

async fn mark_active_deliverable_shipped(app: &AppHandle) -> Result<(), String> {
    let pool = app.state::<AppState>().pool.clone();
    let state = project_manager_shared::repo::menu_bar_state(&pool).await?;
    let Some(id) = state.active_deliverable_id else {
        return Ok(());
    };

    project_manager_shared::repo::update_deliverable_state(
        &pool,
        &id,
        project_manager_shared::models::DeliverableState::Shipped,
    )
    .await?;

    refresh_menu_bar_inner(app).await?;
    show_main_window_inner(app, Some(&format!("/deliverables/{id}")))
}

fn tooltip_for_state(state: &MenuBarState) -> String {
    state
        .active_deliverable_title
        .as_deref()
        .map(|title| format!("Most pressing: {title}"))
        .unwrap_or_else(|| "No drafting or in-review deliverables".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn truncates_tray_title_to_twenty_four_chars() {
        assert_eq!(
            project_manager_shared::repo::truncate_tray_title("Short title"),
            "Short title"
        );
        assert_eq!(
            project_manager_shared::repo::truncate_tray_title(
                "This deliverable title is very long"
            ),
            "This deliverable titl..."
        );
    }
}
