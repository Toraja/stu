use std::sync::Arc;
use tokio::spawn;

use crate::{
    client::Client,
    config::Config,
    error::{AppError, Result},
    event::{
        AppEventType, CompleteDownloadObjectResult, CompleteInitializeResult,
        CompleteLoadObjectResult, CompleteLoadObjectsResult, CompletePreviewObjectResult, Sender,
    },
    file::{copy_to_clipboard, save_binary, save_error_log},
    if_match,
    object::{AppObjects, BucketItem, FileDetail, Object, ObjectItem, ObjectKey},
    pages::page::{Page, PageStack},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewStateTag {
    Initializing,
    BucketList,
    ObjectList,
    Detail,
    DetailSave,
    CopyDetail,
    Preview,
    PreviewSave,
    Help,
}

pub enum Notification {
    None,
    Info(String),
    Success(String),
    Error(String),
}

pub struct AppViewState {
    pub notification: Notification,
    pub is_loading: bool,

    width: usize,
    height: usize,
}

impl AppViewState {
    fn new(width: usize, height: usize) -> AppViewState {
        AppViewState {
            notification: Notification::None,
            is_loading: true,
            width,
            height,
        }
    }

    pub fn reset_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }
}

pub struct App {
    pub app_view_state: AppViewState,
    pub page_stack: PageStack,
    app_objects: AppObjects,
    client: Option<Arc<Client>>,
    config: Option<Config>,
    tx: Sender,
}

impl App {
    pub fn new(tx: Sender, width: usize, height: usize) -> App {
        App {
            app_view_state: AppViewState::new(width, height),
            app_objects: AppObjects::new(),
            page_stack: PageStack::new(tx.clone()),
            client: None,
            config: None,
            tx,
        }
    }

    pub fn initialize(&mut self, config: Config, client: Client, bucket: Option<String>) {
        self.config = Some(config);
        self.client = Some(Arc::new(client));

        let (client, tx) = self.unwrap_client_tx();
        spawn(async move {
            let buckets = match bucket {
                Some(name) => client.load_bucket(&name).await.map(|b| vec![b]),
                None => client.load_all_buckets().await,
            };
            let result = CompleteInitializeResult::new(buckets);
            tx.send(AppEventType::CompleteInitialize(result));
        });
    }

    pub fn complete_initialize(&mut self, result: Result<CompleteInitializeResult>) {
        match result {
            Ok(CompleteInitializeResult { buckets }) => {
                self.app_objects.set_bucket_items(buckets);

                let bucket_list_page = Page::of_bucket_list(self.bucket_items(), self.tx.clone());
                self.page_stack.pop(); // remove initializing page
                self.page_stack.push(bucket_list_page);
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        }

        if self.bucket_items().len() == 1 {
            // bucket name is specified, or if there is only one bucket, open it.
            // since continues to load object, is_loading is not reset.
            self.bucket_list_move_down();
        } else {
            self.app_view_state.is_loading = false;
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.app_view_state.reset_size(width, height);
    }

    pub fn view_state_tag(&self) -> ViewStateTag {
        match self.page_stack.current_page() {
            Page::Initializing(_) => ViewStateTag::Initializing,
            Page::BucketList(_) => ViewStateTag::BucketList,
            Page::ObjectList(_) => ViewStateTag::ObjectList,
            Page::ObjectDetail(p) => match p.status() {
                (true, false) => ViewStateTag::DetailSave,
                (false, true) => ViewStateTag::CopyDetail,
                _ => ViewStateTag::Detail,
            },
            Page::ObjectPreview(p) => match p.status() {
                true => ViewStateTag::PreviewSave,
                _ => ViewStateTag::Preview,
            },
            Page::Help(_) => ViewStateTag::Help,
        }
    }

    fn current_bucket(&self) -> String {
        let bucket_page = self.page_stack.head().as_bucket_list();
        bucket_page.current_selected_item().name.clone()
    }

    fn current_path(&self) -> Vec<&str> {
        self.page_stack
            .iter()
            .filter_map(|page| if_match! { page: Page::ObjectList(p) => p })
            .map(|page| page.current_selected_item())
            .filter_map(|item| if_match! { item: ObjectItem::Dir { name, .. } => name.as_str() })
            .collect()
    }

    fn current_object_prefix(&self) -> String {
        let mut prefix = String::new();
        for key in &self.current_path() {
            prefix.push_str(key);
            prefix.push('/');
        }
        prefix
    }

    fn current_object_key(&self) -> ObjectKey {
        ObjectKey {
            bucket_name: self.current_bucket(),
            object_path: self.current_path().iter().map(|s| s.to_string()).collect(),
        }
    }

    fn current_object_key_with_name(&self, name: String) -> ObjectKey {
        let mut object_path: Vec<String> =
            self.current_path().iter().map(|s| s.to_string()).collect();
        object_path.push(name);
        ObjectKey {
            bucket_name: self.current_bucket(),
            object_path,
        }
    }

    pub fn bucket_items(&self) -> Vec<BucketItem> {
        self.app_objects.get_bucket_items()
    }

    pub fn current_object_items(&self) -> Vec<ObjectItem> {
        self.app_objects
            .get_object_items(&self.current_object_key())
    }

    pub fn bucket_list_select_next(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_next();
    }

    pub fn object_list_select_next(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_next();
    }

    pub fn copy_detail_select_next(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.select_next_copy_detail_item();
    }

    pub fn bucket_list_select_prev(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_prev();
    }

    pub fn object_list_select_prev(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_prev();
    }

    pub fn copy_detail_select_prev(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.select_prev_copy_detail_item();
    }

    pub fn bucket_list_select_next_page(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_next_page();
    }

    pub fn object_list_select_next_page(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_next_page();
    }

    pub fn bucket_list_select_prev_page(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_prev_page();
    }

    pub fn object_list_select_prev_page(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_prev_page();
    }

    pub fn bucket_list_select_first(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_first();
    }

    pub fn object_list_select_first(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_first();
    }

    pub fn bucket_list_select_last(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_bucket_list();
        // page.select_last();
    }

    pub fn object_list_select_last(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_list();
        // page.select_last();
    }

    pub fn bucket_list_move_down(&mut self) {
        if self.exists_current_objects() {
            let object_list_page =
                Page::of_object_list(self.current_object_items(), self.tx.clone());
            self.page_stack.push(object_list_page);
        } else {
            self.tx.send(AppEventType::LoadObjects);
            self.app_view_state.is_loading = true;
        }
    }

    pub fn object_list_move_down(&mut self) {
        let object_page = self.page_stack.current_page().as_object_list();
        let selected = object_page.current_selected_item().to_owned();

        match selected {
            ObjectItem::File { name, .. } => {
                if self.exists_current_object_detail(&name) {
                    let current_object_key = &self.current_object_key_with_name(name.to_string());
                    let detail = self
                        .app_objects
                        .get_object_detail(current_object_key)
                        .unwrap();
                    let versions = self
                        .app_objects
                        .get_object_versions(current_object_key)
                        .unwrap();

                    let object_detail_page = Page::of_object_detail(
                        detail.clone(),
                        versions.clone(),
                        object_page.object_list().clone(),
                        object_page.list_state(),
                        self.tx.clone(),
                    );
                    self.page_stack.push(object_detail_page);
                } else {
                    self.tx.send(AppEventType::LoadObject);
                    self.app_view_state.is_loading = true;
                }
            }
            ObjectItem::Dir { .. } => {
                if self.exists_current_objects() {
                    let object_list_page =
                        Page::of_object_list(self.current_object_items(), self.tx.clone());
                    self.page_stack.push(object_list_page);
                } else {
                    self.tx.send(AppEventType::LoadObjects);
                    self.app_view_state.is_loading = true;
                }
            }
        }
    }

    pub fn copy_detail_copy_selected_value(&self) {
        let object_detail_page = self.page_stack.current_page().as_object_detail();

        if let Some((name, value)) = object_detail_page.copy_detail_dialog_selected() {
            self.tx.send(AppEventType::CopyToClipboard(name, value));
        }
    }

    fn exists_current_object_detail(&self, object_name: &str) -> bool {
        let key = &self.current_object_key_with_name(object_name.to_string());
        self.app_objects.exists_object_details(key)
    }

    fn exists_current_objects(&self) -> bool {
        self.app_objects
            .exists_object_item(&self.current_object_key())
    }

    pub fn object_list_move_up(&mut self) {
        if self.page_stack.len() == 2 /* bucket list and object list */ && self.bucket_items().len() == 1
        {
            return;
        }
        self.page_stack.pop();
    }

    pub fn detail_close(&mut self) {
        self.page_stack.pop(); // remove detail page
    }

    pub fn copy_detail_close(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.close_copy_detail_dialog();
    }

    pub fn preview_scroll_forward(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_preview();
        // page.scroll_forward();
    }

    pub fn preview_scroll_backward(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_preview();
        // page.scroll_backward();
    }

    pub fn preview_scroll_to_top(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_preview();
        // page.scroll_to_top();
    }

    pub fn preview_scroll_to_end(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_preview();
        // page.scroll_to_end();
    }

    pub fn preview_close(&mut self) {
        self.page_stack.pop(); // remove preview page
    }

    pub fn help_close(&mut self) {
        self.toggle_help();
    }

    pub fn object_list_back_to_bucket_list(&mut self) {
        if self.bucket_items().len() == 1 {
            return;
        }
        self.page_stack.clear();
    }

    pub fn load_objects(&self) {
        let bucket = self.current_bucket();
        let prefix = self.current_object_prefix();
        let (client, tx) = self.unwrap_client_tx();
        spawn(async move {
            let items = client.load_objects(&bucket, &prefix).await;
            let result = CompleteLoadObjectsResult::new(items);
            tx.send(AppEventType::CompleteLoadObjects(result));
        });
    }

    pub fn complete_load_objects(&mut self, result: Result<CompleteLoadObjectsResult>) {
        match result {
            Ok(CompleteLoadObjectsResult { items }) => {
                self.app_objects
                    .set_object_items(self.current_object_key().to_owned(), items);

                let object_list_page =
                    Page::of_object_list(self.current_object_items(), self.tx.clone());
                self.page_stack.push(object_list_page);
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        }
        self.app_view_state.is_loading = false;
    }

    pub fn load_object(&self) {
        let object_page = self.page_stack.current_page().as_object_list();

        if let ObjectItem::File {
            name, size_byte, ..
        } = object_page.current_selected_item()
        {
            let name = name.clone();
            let size_byte = *size_byte;

            let bucket = self.current_bucket();
            let prefix = self.current_object_prefix();
            let key = format!("{}{}", prefix, name);

            let map_key = self.current_object_key_with_name(name.to_string());

            let (client, tx) = self.unwrap_client_tx();
            spawn(async move {
                let detail = client
                    .load_object_detail(&bucket, &key, &name, size_byte)
                    .await;
                let versions = client.load_object_versions(&bucket, &key).await;
                let result = CompleteLoadObjectResult::new(detail, versions, map_key);
                tx.send(AppEventType::CompleteLoadObject(result));
            });
        }
    }

    pub fn complete_load_object(&mut self, result: Result<CompleteLoadObjectResult>) {
        match result {
            Ok(CompleteLoadObjectResult {
                detail,
                versions,
                map_key,
            }) => {
                self.app_objects
                    .set_object_details(map_key, *detail.clone(), versions.clone());

                let object_page = self.page_stack.current_page().as_object_list();

                let object_detail_page = Page::of_object_detail(
                    *detail.clone(),
                    versions.clone(),
                    object_page.object_list().clone(),
                    object_page.list_state(),
                    self.tx.clone(),
                );
                self.page_stack.push(object_detail_page);
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        }
        self.app_view_state.is_loading = false;
    }

    pub fn detail_select_tabs(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.toggle_tab();
    }

    pub fn toggle_help(&mut self) {
        match self.view_state_tag() {
            ViewStateTag::Initializing => {}
            ViewStateTag::Help => {
                self.page_stack.pop(); // remove help page
            }
            _ => {
                let helps = match self.page_stack.current_page() {
                    Page::Initializing(page) => page.helps(),
                    Page::BucketList(page) => page.helps(),
                    Page::ObjectList(page) => page.helps(),
                    Page::ObjectDetail(page) => page.helps(),
                    Page::ObjectPreview(page) => page.helps(),
                    Page::Help(page) => page.helps(),
                };
                let help_page = Page::of_help(helps, self.tx.clone());
                self.page_stack.push(help_page);
            }
        }
    }

    pub fn detail_download_object(&mut self) {
        let object_detail_page = self.page_stack.current_page().as_object_detail();
        let file_detail = object_detail_page.file_detail();

        self.tx
            .send(AppEventType::DownloadObject(file_detail.clone()));
        self.app_view_state.is_loading = true;
    }

    pub fn detail_open_download_object_as(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.open_save_dialog();
    }

    pub fn preview_download_object(&self) {
        let object_preview_page = self.page_stack.current_page().as_object_preview();

        // object has been already downloaded, so send completion event to save file
        let obj = object_preview_page.object();
        let path = object_preview_page.path();
        let result = CompleteDownloadObjectResult::new(Ok(obj.clone()), path.to_string());
        self.tx.send(AppEventType::CompleteDownloadObject(result));
    }

    pub fn preview_open_download_object_as(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_preview();
        // page.open_save_dialog();
    }

    pub fn detail_preview(&mut self) {
        let object_detail_page = self.page_stack.current_page().as_object_detail();
        let file_detail = object_detail_page.file_detail();

        self.tx
            .send(AppEventType::PreviewObject(file_detail.clone()));
        self.app_view_state.is_loading = true;
    }

    pub fn detail_open_copy_details(&mut self) {
        // let page = self.page_stack.current_page_mut().as_mut_object_detail();
        // page.open_copy_detail_dialog();
    }

    pub fn download_object(&self, file_detail: FileDetail) {
        let object_name = file_detail.name;
        let size_byte = file_detail.size_byte;

        self.download_object_and(&object_name, size_byte, None, |tx, obj, path| {
            let result = CompleteDownloadObjectResult::new(obj, path);
            tx.send(AppEventType::CompleteDownloadObject(result));
        })
    }

    pub fn download_object_as(&self, file_detail: FileDetail, input: String) {
        let object_name = file_detail.name;
        let size_byte = file_detail.size_byte;

        self.download_object_and(&object_name, size_byte, Some(&input), |tx, obj, path| {
            let result = CompleteDownloadObjectResult::new(obj, path);
            tx.send(AppEventType::CompleteDownloadObject(result));
        })
    }

    pub fn complete_download_object(&mut self, result: Result<CompleteDownloadObjectResult>) {
        let result = match result {
            Ok(CompleteDownloadObjectResult { obj, path }) => {
                save_binary(&path, &obj.bytes).map(|_| path)
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(path) => {
                let msg = format!("Download completed successfully: {}", path);
                self.tx.send(AppEventType::NotifySuccess(msg));
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        }
        self.app_view_state.is_loading = false;
    }

    pub fn preview_object(&self, file_detail: FileDetail) {
        let object_name = file_detail.name.clone();
        let size_byte = file_detail.size_byte;

        self.download_object_and(&object_name, size_byte, None, |tx, obj, path| {
            let result = CompletePreviewObjectResult::new(obj, file_detail, path);
            tx.send(AppEventType::CompletePreviewObject(result));
        })
    }

    pub fn complete_preview_object(&mut self, result: Result<CompletePreviewObjectResult>) {
        match result {
            Ok(CompletePreviewObjectResult {
                obj,
                file_detail,
                path,
            }) => {
                let object_preview_page =
                    Page::of_object_preview(file_detail, obj, path, self.tx.clone());
                self.page_stack.push(object_preview_page);
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        };
        self.clear_notification();
        self.app_view_state.is_loading = false;
    }

    fn download_object_and<F>(
        &self,
        object_name: &str,
        size_byte: usize,
        save_file_name: Option<&str>,
        f: F,
    ) where
        F: FnOnce(Sender, Result<Object>, String) + Send + 'static,
    {
        let bucket = self.current_bucket();
        let prefix = self.current_object_prefix();
        let key = format!("{}{}", prefix, object_name);

        let config = self.config.as_ref().unwrap();
        let path = config.download_file_path(save_file_name.unwrap_or(object_name));

        let (client, tx) = self.unwrap_client_tx();
        let loading = self.handle_loading_size(size_byte, tx.clone());
        spawn(async move {
            let obj = client
                .download_object(&bucket, &key, size_byte, loading)
                .await;
            f(tx, obj, path);
        });
    }

    fn handle_loading_size(&self, total_size: usize, tx: Sender) -> Box<dyn Fn(usize) + Send> {
        if total_size < 10_000_000 {
            return Box::new(|_| {});
        }
        let decimal_places = if total_size > 1_000_000_000 { 1 } else { 0 };
        let opt =
            humansize::FormatSizeOptions::from(humansize::DECIMAL).decimal_places(decimal_places);
        let total_s = humansize::format_size_i(total_size, opt);
        let f = move |current| {
            let percent = (current * 100) / total_size;
            let cur_s = humansize::format_size_i(current, opt);
            let msg = format!("{:3}% downloaded ({} out of {})", percent, cur_s, total_s);
            tx.send(AppEventType::NotifyInfo(msg));
        };
        Box::new(f)
    }

    pub fn bucket_list_open_management_console(&self) {
        let (client, _) = self.unwrap_client_tx();
        let result = client.open_management_console_buckets();
        if let Err(e) = result {
            self.tx.send(AppEventType::NotifyError(e));
        }
    }

    pub fn object_list_open_management_console(&self) {
        let (client, _) = self.unwrap_client_tx();
        let bucket = &self.current_bucket();
        let prefix = self.current_object_prefix();
        let result = client.open_management_console_list(bucket, &prefix);
        if let Err(e) = result {
            self.tx.send(AppEventType::NotifyError(e));
        }
    }

    pub fn detail_open_management_console(&self) {
        let object_detail_page = self.page_stack.current_page().as_object_detail();

        let (client, _) = self.unwrap_client_tx();
        let prefix = self.current_object_prefix();

        let result = client.open_management_console_object(
            &self.current_bucket(),
            &prefix,
            &object_detail_page.file_detail().name,
        );
        if let Err(e) = result {
            self.tx.send(AppEventType::NotifyError(e));
        }
    }

    pub fn detail_save_download_object_as(&mut self) {
        let object_detail_page = self.page_stack.current_page().as_object_detail();
        let file_detail = object_detail_page.file_detail();

        if let Some(input) = object_detail_page.save_dialog_key_input() {
            let input = input.trim().to_string();
            if !input.is_empty() {
                self.tx
                    .send(AppEventType::DownloadObjectAs(file_detail.clone(), input));
                self.app_view_state.is_loading = true;
            }

            let page = self.page_stack.current_page_mut().as_mut_object_detail();
            page.close_save_dialog();
        }
    }

    pub fn preview_save_download_object_as(&mut self) {
        let object_preview_page = self.page_stack.current_page().as_object_preview();
        let file_detail = object_preview_page.file_detail();

        if let Some(input) = object_preview_page.save_dialog_key_input() {
            let input = input.trim().to_string();
            if !input.is_empty() {
                self.tx
                    .send(AppEventType::DownloadObjectAs(file_detail.clone(), input));
                self.app_view_state.is_loading = true;
            }

            let page = self.page_stack.current_page_mut().as_mut_object_preview();
            page.close_save_dialog();
        }
    }

    pub fn copy_to_clipboard(&self, name: String, value: String) {
        match copy_to_clipboard(value) {
            Ok(_) => {
                let msg = format!("Copied '{}' to clipboard successfully", name);
                self.tx.send(AppEventType::NotifySuccess(msg));
            }
            Err(e) => {
                self.tx.send(AppEventType::NotifyError(e));
            }
        }
    }

    pub fn clear_notification(&mut self) {
        self.app_view_state.notification = Notification::None;
    }

    pub fn info_notification(&mut self, msg: String) {
        self.app_view_state.notification = Notification::Info(msg);
    }

    pub fn success_notification(&mut self, msg: String) {
        self.app_view_state.notification = Notification::Success(msg);
    }

    pub fn error_notification(&mut self, e: AppError) {
        self.save_error(&e);
        self.app_view_state.notification = Notification::Error(e.msg);
    }

    fn save_error(&self, e: &AppError) {
        let config = self.config.as_ref().unwrap();
        // cause panic if save errors
        let path = config.error_log_path().unwrap();
        save_error_log(&path, e).unwrap();
    }

    fn unwrap_client_tx(&self) -> (Arc<Client>, Sender) {
        (self.client.as_ref().unwrap().clone(), self.tx.clone())
    }
}
