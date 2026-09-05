import { invoke } from "@tauri-apps/api/core";

export type FileEntityKind =
  | "initiative"
  | "deliverable"
  | "deliverable_task"
  | "stakeholder"
  | "capture"
  | "meeting"
  | "conversation";

export const FILE_ENTITY_KIND_LABELS: Record<FileEntityKind, string> = {
  initiative: "Initiative",
  deliverable: "Deliverable",
  deliverable_task: "Task",
  stakeholder: "Stakeholder",
  capture: "Capture",
  meeting: "Meeting",
  conversation: "Conversation",
};

export interface TraceFolder {
  id: string;
  parent_id: string | null;
  name: string;
  created_at: string;
  updated_at: string;
}

export interface TraceFolderWithLinks {
  id: string;
  parent_id: string | null;
  name: string;
  created_at: string;
  updated_at: string;
  fileCount: number;
  links: FileLinkRef[];
}

export interface FileLinkRef {
  entityKind: FileEntityKind;
  entityId: string;
  linkedAt: string;
}

export interface FileRow {
  id: string;
  kind: "local" | "drive";
  traceFolderId: string | null;
  name: string;
  mimeType: string | null;
  sizeBytes: number | null;
  description: string | null;
  localPath: string | null;
  isMissing: boolean;
  driveFileId: string | null;
  driveAccountId: string | null;
  driveParentId: string | null;
  driveMime: string | null;
  driveWebViewLink: string | null;
  driveTrashed: boolean;
  createdAt: string;
  updatedAt: string;
  links: FileLinkRef[];
}

export interface FolderListing {
  folder: TraceFolder | null;
  breadcrumbs: TraceFolder[];
  folders: TraceFolder[];
  files: FileRow[];
}

function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

function ipc<T>(command: string, args?: Record<string, unknown>) {
  if (!isTauriRuntime()) {
    return Promise.reject(
      "Files commands are available in the macOS app. Open the Tauri window for live data.",
    );
  }
  return invoke<T>(command, args);
}

export function listTraceFolders(parentId: string | null) {
  return ipc<TraceFolder[]>("list_trace_folders", { parentId });
}

export function listFolderChildren(folderId: string | null) {
  return ipc<FolderListing>("list_folder_children", { folderId });
}

export function createTraceFolder(parentId: string | null, name: string) {
  return ipc<TraceFolder>("create_trace_folder", { input: { parent_id: parentId, name } });
}

export function renameTraceFolder(id: string, name: string) {
  return ipc<void>("rename_trace_folder", { input: { id, name } });
}

export function moveTraceFolder(id: string, newParentId: string | null) {
  return ipc<void>("move_trace_folder", { input: { id, new_parent_id: newParentId } });
}

export function deleteTraceFolder(id: string, cascade = false) {
  return ipc<void>("delete_trace_folder", { id, cascade });
}

export function attachLocalFile(path: string, traceFolderId: string | null = null) {
  return ipc<FileRow>("attach_local_file", {
    input: { path, trace_folder_id: traceFolderId },
  });
}

export function attachLocalDirectory(path: string, traceFolderId: string | null = null) {
  return ipc<TraceFolder>("attach_local_directory", { path, traceFolderId });
}

export function moveFile(fileId: string, newTraceFolderId: string | null) {
  return ipc<void>("move_file", {
    input: { file_id: fileId, new_trace_folder_id: newTraceFolderId },
  });
}

export function renameFile(fileId: string, name: string) {
  return ipc<void>("rename_file", { input: { file_id: fileId, name } });
}

export function deleteFile(id: string) {
  return ipc<void>("delete_file", { id });
}

export function linkFileToEntity(
  fileId: string,
  entityKind: FileEntityKind,
  entityId: string,
) {
  return ipc<void>("link_file_to_entity", {
    input: { file_id: fileId, entity_kind: entityKind, entity_id: entityId },
  });
}

export function unlinkFileFromEntity(
  fileId: string,
  entityKind: FileEntityKind,
  entityId: string,
) {
  return ipc<void>("unlink_file_from_entity", {
    input: { file_id: fileId, entity_kind: entityKind, entity_id: entityId },
  });
}

export function filesForEntity(entityKind: FileEntityKind, entityId: string) {
  return ipc<FileRow[]>("files_for_entity", {
    input: { entity_kind: entityKind, entity_id: entityId },
  });
}

export function linkFolderFilesToEntity(
  folderId: string,
  entityKind: FileEntityKind,
  entityId: string,
) {
  return ipc<number>("link_folder_files_to_entity", {
    input: { folder_id: folderId, entity_kind: entityKind, entity_id: entityId },
  });
}

export function linkFolderToEntity(
  folderId: string,
  entityKind: FileEntityKind,
  entityId: string,
) {
  return ipc<void>("link_folder_to_entity", {
    input: { folder_id: folderId, entity_kind: entityKind, entity_id: entityId },
  });
}

export function unlinkFolderFromEntity(
  folderId: string,
  entityKind: FileEntityKind,
  entityId: string,
) {
  return ipc<void>("unlink_folder_from_entity", {
    input: { folder_id: folderId, entity_kind: entityKind, entity_id: entityId },
  });
}

export function getFolderLinks(folderId: string) {
  return ipc<FileLinkRef[]>("folder_links", { folderId });
}

export function foldersForEntity(entityKind: FileEntityKind, entityId: string) {
  return ipc<TraceFolderWithLinks[]>("folders_for_entity", {
    input: { entity_kind: entityKind, entity_id: entityId },
  });
}

export function openFile(id: string) {
  return ipc<void>("open_file", { id });
}

export function searchFiles(query: string, limit = 50) {
  return ipc<FileRow[]>("search_files", { query, limit });
}

/**
 * Ensures the canonical Trace folder hierarchy for the given entity exists
 * and returns the leaf folder. Hierarchy:
 *   initiative        → {initiative.title}/
 *   deliverable       → {initiative.title}/{deliverable.title}/
 *   deliverable_task  → {initiative.title}/{deliverable.title}/{task.title}/
 *   stakeholder       → {stakeholder.name}/
 */
export function resolveEntityFolder(entityKind: FileEntityKind, entityId: string) {
  return ipc<TraceFolder>("resolve_entity_folder", { entityKind, entityId });
}

// ---- Google Drive ----

export interface DriveAccount {
  id: string;
  email: string;
  createdAt: string;
}

export interface DriveStatus {
  connected: boolean;
  accounts: DriveAccount[];
}

export interface DriveEntry {
  id: string;
  name: string;
  mimeType: string;
  isFolder: boolean;
  webViewLink: string | null;
  modifiedTime: string | null;
  size: number | null;
  parents: string[];
}

export interface DriveListing {
  entries: DriveEntry[];
  nextPageToken: string | null;
}

export function driveStatus() {
  return ipc<DriveStatus>("drive_status");
}

export function driveConnect() {
  return ipc<DriveAccount>("drive_connect");
}

export function driveDisconnect(accountId: string) {
  return ipc<void>("drive_disconnect", { accountId });
}

export function driveListChildren(parentId: string | null, pageToken: string | null = null) {
  return ipc<DriveListing>("drive_list_children", { parentId, pageToken });
}

export function driveImport(
  accountId: string,
  driveFileIds: string[],
  traceFolderId: string | null = null,
) {
  return ipc<FileRow[]>("drive_import", {
    accountId,
    driveFileIds,
    traceFolderId,
  });
}

export function driveImportFolder(
  accountId: string,
  driveFolderId: string,
  folderName: string,
  traceFolderId: string | null = null,
) {
  return ipc<TraceFolder>("drive_import_folder", {
    accountId,
    driveFolderId,
    folderName,
    traceFolderId,
  });
}

export interface DriveSyncStatus {
  lastSyncAt: string | null;
  initialized: boolean;
}

export function drivePullChanges(accountId: string) {
  return ipc<number>("drive_pull_changes", { accountId });
}

export function driveSyncStatus(accountId: string) {
  return ipc<DriveSyncStatus>("drive_sync_status", { accountId });
}

export function disconnectGoogleDrive(accountId: string): Promise<void> {
  return ipc<void>("drive_disconnect", { accountId });
}

export function openDrivePreview(
  fileId: string,
  driveFileId: string,
  driveMime: string,
  title: string,
): Promise<void> {
  return ipc<void>("open_drive_preview", { fileId, driveFileId, driveMime, title });
}

// ── GMeet transcript folder ────────────────────────────────────────────────────

export interface GmeetFolder {
  accountId: string;
  folderId: string;
  folderName: string;
}

export function getGmeetFolder(accountId: string): Promise<GmeetFolder | null> {
  return ipc<GmeetFolder | null>("get_gmeet_folder", { accountId });
}

export function setGmeetFolder(
  accountId: string,
  folderId: string,
  folderName: string,
): Promise<void> {
  return ipc<void>("set_gmeet_folder", { accountId, folderId, folderName });
}

export function clearGmeetFolder(accountId: string): Promise<void> {
  return ipc<void>("clear_gmeet_folder", { accountId });
}

// ── Drive transcript import ────────────────────────────────────────────────────

export function importDriveTranscript(
  driveFileId: string,
  fileName: string,
  stakeholderIds: string[],
): Promise<import("./types").MeetingWithActions> {
  return ipc<import("./types").MeetingWithActions>("import_drive_transcript", {
    driveFileId,
    fileName,
    stakeholderIds,
  });
}

/** Returns true for the three Google Workspace MIME types that support in-app preview. */
export function isPreviewableDriveMime(mime: string | null | undefined): boolean {
  return (
    mime === "application/vnd.google-apps.document" ||
    mime === "application/vnd.google-apps.spreadsheet" ||
    mime === "application/vnd.google-apps.presentation"
  );
}

// ── Google Docs / Slides editor IPC ──────────────────────────────────────────

export type { GDocsDocument } from "./docsConverter";
import type { GDocsDocument } from "./docsConverter";

export function getGoogleDoc(driveFileId: string): Promise<GDocsDocument> {
  return ipc<GDocsDocument>("get_google_doc", { driveFileId });
}

export function saveGoogleDoc(driveFileId: string, requests: object[]): Promise<void> {
  return ipc<void>("save_google_doc", { driveFileId, requests });
}

export function getGoogleSlides(driveFileId: string): Promise<unknown> {
  return ipc<unknown>("get_google_slides", { driveFileId });
}

export function getSlideThumbnail(presentationId: string, pageObjectId: string): Promise<string> {
  return ipc<string>("get_slide_thumbnail", { presentationId, pageObjectId });
}

export function getGoogleSheet(driveFileId: string): Promise<unknown> {
  return ipc<unknown>("get_google_sheet", { driveFileId });
}

/** True if the stored Drive token includes the documents + presentations scopes. */
export function driveHasEditorScope(): Promise<boolean> {
  return ipc<boolean>("drive_has_editor_scope");
}

export interface DriveFileMetadata {
  id: string;
  name: string;
  mimeType: string;
  webViewLink?: string | null;
  modifiedTime?: string | null;
  size?: string | null;
  parents?: string[];
  trashed?: boolean;
  md5Checksum?: string | null;
}

/** Fetch file metadata (name + mimeType) for a Drive file by ID. */
export function driveGetFileMetadata(fileId: string): Promise<DriveFileMetadata> {
  return ipc<DriveFileMetadata>("drive_get_file_metadata", { fileId });
}
