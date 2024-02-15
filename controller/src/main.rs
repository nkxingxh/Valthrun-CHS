#![allow(dead_code)]
#![feature(const_fn_floating_point_arithmetic)]

use std::{
    cell::{
        RefCell,
        RefMut,
    },
    error::Error,
    fmt::Debug,
    fs::File,
    io::BufWriter,
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
    time::{
        Duration,
        Instant,
    },
};

use anyhow::Context;
use cache::EntryCache;
use clap::{
    Args,
    Parser,
    Subcommand,
};
use cs2::{
    offsets_runtime,
    BuildInfo,
    CS2Handle,
    CS2Model,
    CS2Offsets,
    ClassNameCache,
    EntitySystem,
    Globals,
};
use enhancements::Enhancement;
use imgui::{
    Condition,
    FontConfig,
    FontId,
    FontSource,
    Ui,
};
use obfstr::obfstr;
use overlay::{
    LoadingError,
    OverlayError,
    OverlayOptions,
    OverlayTarget,
    SystemRuntimeController,
};
use settings::{
    load_app_settings,
    AppSettings,
    SettingsUI,
};
use valthrun_kernel_interface::KInterfaceError;
use view::ViewController;
use windows::Win32::{
    System::Console::GetConsoleProcessList,
    UI::Shell::IsUserAnAdmin,
};

use crate::{
    enhancements::{
        AntiAimPunsh,
        BombInfo,
        PlayerESP,
        SpectatorsList,
        TriggerBot,
    },
    settings::save_app_settings,
    view::LocalCrosshair,
    winver::version_info,
};

mod cache;
mod enhancements;
mod settings;
mod utils;
mod view;
mod winver;

pub trait MetricsClient {
    fn add_metrics_record(&self, record_type: &str, record_payload: &str);
}

impl MetricsClient for CS2Handle {
    fn add_metrics_record(&self, record_type: &str, record_payload: &str) {
        self.add_metrics_record(record_type, record_payload)
    }
}

pub trait KeyboardInput {
    fn is_key_down(&self, key: imgui::Key) -> bool;
    fn is_key_pressed(&self, key: imgui::Key, repeating: bool) -> bool;
}

impl KeyboardInput for imgui::Ui {
    fn is_key_down(&self, key: imgui::Key) -> bool {
        Ui::is_key_down(self, key)
    }

    fn is_key_pressed(&self, key: imgui::Key, repeating: bool) -> bool {
        if repeating {
            Ui::is_key_pressed(self, key)
        } else {
            Ui::is_key_pressed_no_repeat(self, key)
        }
    }
}

pub struct UpdateContext<'a> {
    pub settings: &'a AppSettings,
    pub input: &'a dyn KeyboardInput,

    pub cs2: &'a Arc<CS2Handle>,
    pub cs2_entities: &'a EntitySystem,

    pub model_cache: &'a EntryCache<u64, CS2Model>,
    pub class_name_cache: &'a ClassNameCache,
    pub view_controller: &'a ViewController,

    pub globals: Globals,
}

pub struct AppFonts {
    valthrun: FontId,
}

pub struct Application {
    pub fonts: AppFonts,

    pub cs2: Arc<CS2Handle>,
    pub cs2_offsets: Arc<CS2Offsets>,
    pub cs2_entities: EntitySystem,
    pub cs2_globals: Option<Globals>,
    pub cs2_build_info: BuildInfo,

    pub model_cache: EntryCache<u64, CS2Model>,
    pub class_name_cache: ClassNameCache,
    pub view_controller: ViewController,

    pub enhancements: Vec<Rc<RefCell<dyn Enhancement>>>,

    pub frame_read_calls: usize,
    pub last_total_read_calls: usize,

    pub settings: Rc<RefCell<AppSettings>>,
    pub settings_visible: bool,
    pub settings_dirty: bool,
    pub settings_ui: RefCell<SettingsUI>,
    pub settings_screen_capture_changed: AtomicBool,
    pub settings_render_debug_window_changed: AtomicBool,
}

impl Application {
    pub fn settings(&self) -> std::cell::Ref<'_, AppSettings> {
        self.settings.borrow()
    }

    pub fn settings_mut(&self) -> RefMut<'_, AppSettings> {
        self.settings.borrow_mut()
    }

    pub fn pre_update(&mut self, controller: &mut SystemRuntimeController) -> anyhow::Result<()> {
        if self.settings_dirty {
            self.settings_dirty = false;
            let mut settings = self.settings.borrow_mut();

            settings.imgui = None;
            if let Ok(value) = serde_json::to_string(&*settings) {
                self.cs2.add_metrics_record("settings-updated", &value);
            }

            let mut imgui_settings = String::new();
            controller.imgui.save_ini_settings(&mut imgui_settings);
            settings.imgui = Some(imgui_settings);

            if let Err(error) = save_app_settings(&*settings) {
                log::warn!("保存用户设置失败: {}", error);
            };
        }

        if self
            .settings_screen_capture_changed
            .swap(false, Ordering::Relaxed)
        {
            let settings = self.settings.borrow();
            controller.toggle_screen_capture_visibility(!settings.hide_overlay_from_screen_capture);
            log::debug!(
                "将屏幕截图的可见性更新至 {}",
                !settings.hide_overlay_from_screen_capture
            );
        }

        if self
            .settings_render_debug_window_changed
            .swap(false, Ordering::Relaxed)
        {
            let settings = self.settings.borrow();
            controller.toggle_debug_overlay(settings.render_debug_window);
        }

        Ok(())
    }

    pub fn update(&mut self, ui: &imgui::Ui) -> anyhow::Result<()> {
        {
            let mut settings = self.settings.borrow_mut();
            for enhancement in self.enhancements.iter() {
                let mut hack = enhancement.borrow_mut();
                if hack.update_settings(ui, &mut *settings)? {
                    self.settings_dirty = true;
                }
            }
        }

        let settings = self.settings.borrow();
        if ui.is_key_pressed_no_repeat(settings.key_settings.0) {
            log::debug!("Toogle settings");
            self.settings_visible = !self.settings_visible;
            self.cs2.add_metrics_record(
                "settings-toggled",
                &format!("visible: {}", self.settings_visible),
            );

            if !self.settings_visible {
                /* overlay has just been closed */
                self.settings_dirty = true;
            }
        }

        self.view_controller
            .update_screen_bounds(mint::Vector2::from_slice(&ui.io().display_size));
        self.view_controller.update_view_matrix(&self.cs2)?;

        let globals = self
            .cs2
            .reference_schema::<Globals>(&[self.cs2_offsets.globals, 0])?
            .cached()
            .with_context(|| obfstr!("未能读取全局数据").to_string())?;

        self.cs2_entities
            .read_entities()
            .with_context(|| obfstr!("无法读取全局实体列表").to_string())?;

        self.class_name_cache
            .update_cache(self.cs2_entities.all_identities())
            .with_context(|| obfstr!("更新类名缓存失败").to_string())?;

        let update_context = UpdateContext {
            cs2: &self.cs2,
            cs2_entities: &self.cs2_entities,

            settings: &*settings,
            input: ui,

            globals,
            class_name_cache: &self.class_name_cache,
            view_controller: &self.view_controller,
            model_cache: &self.model_cache,
        };

        for enhancement in self.enhancements.iter() {
            let mut hack = enhancement.borrow_mut();
            hack.update(&update_context)?;
        }

        let read_calls = self.cs2.ke_interface.total_read_calls();
        self.frame_read_calls = read_calls - self.last_total_read_calls;
        self.last_total_read_calls = read_calls;

        Ok(())
    }

    pub fn render(&self, ui: &imgui::Ui) {
        ui.window("overlay")
            .draw_background(false)
            .no_decoration()
            .no_inputs()
            .size(ui.io().display_size, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .build(|| self.render_overlay(ui));

        {
            let mut settings = self.settings.borrow_mut();
            for enhancement in self.enhancements.iter() {
                let mut enhancement = enhancement.borrow_mut();
                enhancement.render_debug_window(&mut *settings, ui);
            }
        }

        if self.settings_visible {
            let mut settings_ui = self.settings_ui.borrow_mut();
            settings_ui.render(self, ui)
        }
    }

    fn render_overlay(&self, ui: &imgui::Ui) {
        let settings = self.settings.borrow();

        if settings.valthrun_watermark {
            {
                let text_buf;
                let text = obfstr!(text_buf = "Valthrun-CHS 叠加层");

                ui.set_cursor_pos([
                    ui.window_size()[0] - ui.calc_text_size(text)[0] - 10.0,
                    10.0,
                ]);
                ui.text(text);
            }
            {
                let current_fps = ui.io().framerate;
                if settings.overlay_fps_limit > 0 && current_fps as u32 > settings.overlay_fps_limit
                {
                    let duration = std::time::Duration::from_millis(
                        ((1000.0 / current_fps) * (current_fps - settings.overlay_fps_limit as f32))
                            as u64,
                    );
                    std::thread::sleep(duration);
                }
                let text = format!("{:.2} FPS", current_fps);
                ui.set_cursor_pos([
                    ui.window_size()[0] - ui.calc_text_size(&text)[0] - 10.0,
                    24.0,
                ]);
                ui.text(text)
            }
            {
                let text = format!("{} Reads", self.frame_read_calls);
                ui.set_cursor_pos([
                    ui.window_size()[0] - ui.calc_text_size(&text)[0] - 10.0,
                    38.0,
                ]);
                ui.text(text)
            }
        }

        for hack in self.enhancements.iter() {
            let hack = hack.borrow();
            hack.render(&*settings, ui, &self.view_controller);
        }
    }
}

fn show_critical_error(message: &str) {
    for line in message.lines() {
        log::error!("{}", line);
    }

    if !is_console_invoked() {
        overlay::show_error_message(obfstr!("Valthrun-CHS 控制器"), message);
    }
}

fn main() {
    let args = match AppArgs::try_parse() {
        Ok(args) => args,
        Err(error) => {
            println!("{:#}", error);
            std::process::exit(1);
        }
    };

    env_logger::builder()
        .filter_level(if args.verbose {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        })
        .parse_default_env()
        .init();

    let command = args.command.as_ref().unwrap_or(&AppCommand::Overlay);
    let result = match command {
        AppCommand::DumpSchema(args) => main_schema_dump(args),
        AppCommand::Overlay => main_overlay(),
    };

    if let Err(error) = result {
        show_critical_error(&format!("{:#}", error));
    }
}

#[derive(Debug, Parser)]
#[clap(name = "Valthrun", version)]
struct AppArgs {
    /// Enable verbose logging ($env:RUST_LOG="trace")
    #[clap(short, long)]
    verbose: bool,

    #[clap(subcommand)]
    command: Option<AppCommand>,
}

#[derive(Debug, Subcommand)]
enum AppCommand {
    /// Start the overlay
    Overlay,

    /// Create a schema dump
    DumpSchema(SchemaDumpArgs),
}

#[derive(Debug, Args)]
struct SchemaDumpArgs {
    pub target_file: PathBuf,
}

fn is_console_invoked() -> bool {
    let console_count = unsafe {
        let mut result = [0u32; 128];
        GetConsoleProcessList(&mut result)
    };

    console_count > 1
}

fn main_schema_dump(args: &SchemaDumpArgs) -> anyhow::Result<()> {
    log::info!("正在转储模式 (schema)。请稍候...");

    let cs2 = CS2Handle::create(true)?;
    let schema = cs2::dump_schema(&cs2, false)?;

    let output = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&args.target_file)?;

    let mut output = BufWriter::new(output);
    serde_json::to_writer_pretty(&mut output, &schema)?;
    log::info!("模式已转储到 {}", args.target_file.to_string_lossy());
    Ok(())
}

fn main_overlay() -> anyhow::Result<()> {
    let build_info = version_info()?;
    log::info!(
        "{} 版本 {} ({})，Windows 内部版本 {}。",
        obfstr!("Valthrun-CHS"),
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        build_info.dwBuildNumber
    );
    log::info!(
        "{} {} 构建。",
        obfstr!("当前可执行文件于"),
        env!("BUILD_TIME")
    );

    if unsafe { IsUserAnAdmin().as_bool() } {
        log::warn!(
            "{}",
            obfstr!("当前以管理员身份运行，可能会导致图形驱动程序出现故障。")
        );
    }

    let settings = load_app_settings()?;
    let cs2 = match CS2Handle::create(settings.metrics) {
        Ok(handle) => handle,
        Err(err) => {
            if let Some(err) = err.downcast_ref::<KInterfaceError>() {
                if let KInterfaceError::DeviceUnavailable(error) = &err {
                    if error.code().0 as u32 == 0x80070002 {
                        /* The system cannot find the file specified. */
                        show_critical_error(obfstr!("** 请仔细阅读 **\n无法找到内核驱动程序接口。\n在启动控制器之前，请确保已成功加载或映射内核驱动程序 (valthrun-driver.sys)。请明确检查驱动程序入口状态代码，该代码应为 0x0。\n\n如需更多帮助，请查阅文档中的疑难解答部分: \nhttps://wiki.valth.run/#/zh-cn/"));
                        return Ok(());
                    }
                } else if let KInterfaceError::DriverTooOld {
                    driver_version_string,
                    requested_version_string,
                    ..
                } = &err
                {
                    let message = obfstr!(
                        "\n已加载的 Valthrun-CHS 驱动程序版本太低。\n请确保已加载对应当前版本的驱动程序。\n注意: 如果手动映射了驱动程序，则需要先卸载驱动才能加载新版本。如果你使用的驱动映射器不支持卸载驱动，请重启计算机。"
                    ).to_string();

                    show_critical_error(&format!(
                        "{}\n\n已加载驱动版本: {}\n需要驱动版本: {}",
                        message, driver_version_string, requested_version_string
                    ));
                    return Ok(());
                } else if let KInterfaceError::DriverTooNew {
                    driver_version_string,
                    requested_version_string,
                    ..
                } = &err
                {
                    let message = obfstr!(
                        "\n已加载的 Valthrun-CHS 驱动程序版本太高。\n请确保你使用了对应驱动版本的控制器。"
                    ).to_string();

                    show_critical_error(&format!(
                        "{}\n\n已加载驱动版本: {}\n需要驱动版本: {}",
                        message, driver_version_string, requested_version_string
                    ));
                    return Ok(());
                } else if let KInterfaceError::ProcessDoesNotExists = &err {
                    show_critical_error(obfstr!(
                        "无法找到游戏进程。\n请在启动本程序前先启动游戏！"
                    ));
                    return Ok(());
                }
            }

            return Err(err);
        }
    };

    cs2.add_metrics_record(obfstr!("controller-status"), "initializing");

    let cs2_build_info = BuildInfo::read_build_info(&cs2).with_context(|| {
        obfstr!("加载 CS2 构建信息失败。CS2 版本可能高于或低于预期").to_string()
    })?;
    log::info!(
        "已找到 {} 修订版本 {} 来自 {}.",
        obfstr!("Counter-Strike 2"),
        cs2_build_info.revision,
        cs2_build_info.build_datetime
    );
    cs2.add_metrics_record(
        obfstr!("cs2-version"),
        &format!("revision: {}", cs2_build_info.revision),
    );

    offsets_runtime::setup_provider(&cs2)?;
    let cs2_offsets = Arc::new(
        CS2Offsets::resolve_offsets(&cs2)
            .with_context(|| obfstr!("无法加载 CS2 偏移量").to_string())?,
    );

    let imgui_settings = settings.imgui.clone();
    let settings = Rc::new(RefCell::new(settings));

    log::debug!("初始化叠加层");
    let app_fonts: Rc<RefCell<Option<AppFonts>>> = Default::default();
    let overlay_options = OverlayOptions {
        title: obfstr!("C2OL").to_string(),
        target: OverlayTarget::WindowOfProcess(cs2.process_id() as u32),
        font_init: Some(Box::new({
            let app_fonts = app_fonts.clone();

            move |imgui| {
                let mut app_fonts = app_fonts.borrow_mut();

                let font_size = 18.0;
                let valthrun_font = imgui.fonts().add_font(&[FontSource::TtfData {
                    data: include_bytes!("../resources/Valthrun-Regular.ttf"),
                    size_pixels: font_size,
                    config: Some(FontConfig {
                        rasterizer_multiply: 1.5,
                        oversample_h: 4,
                        oversample_v: 4,
                        ..FontConfig::default()
                    }),
                }]);

                *app_fonts = Some(AppFonts {
                    valthrun: valthrun_font,
                });
            }
        })),
    };

    let mut overlay = match overlay::init(&overlay_options) {
        Err(OverlayError::VulkanDllNotFound(LoadingError::LibraryLoadFailure(source))) => {
            match &source {
                libloading::Error::LoadLibraryExW { .. } => {
                    let error = source.source().context("LoadLibraryExW to have a source")?;
                    let message = format!("加载 vulkan-1.dll 失败。\n错误: {:#}", error);
                    show_critical_error(&message);
                }
                error => {
                    let message = format!("加载 vulkan-1.dll 时发生错误。\n错误: {:#}", error);
                    show_critical_error(&message);
                }
            }
            return Ok(());
        }
        value => value?,
    };
    if let Some(imgui_settings) = imgui_settings {
        overlay.imgui.load_ini_settings(&imgui_settings);
    }

    let app = Application {
        fonts: app_fonts
            .borrow_mut()
            .take()
            .context("初始化应用程序字体失败")?,

        cs2: cs2.clone(),
        cs2_entities: EntitySystem::new(cs2.clone(), cs2_offsets.clone()),
        cs2_offsets: cs2_offsets.clone(),
        cs2_globals: None,
        cs2_build_info,

        model_cache: EntryCache::new({
            let cs2 = cs2.clone();
            move |model| {
                let model_name = cs2.read_string(&[*model as u64 + 0x08, 0], Some(32))?;
                log::debug!(
                    "{} {} 位于 {:X}。 缓存中...",
                    obfstr!("发现新的玩家模型"),
                    model_name,
                    model
                );

                Ok(CS2Model::read(&cs2, *model as u64)?)
            }
        }),
        class_name_cache: ClassNameCache::new(cs2.clone()),
        view_controller: ViewController::new(cs2_offsets.clone()),

        enhancements: vec![
            Rc::new(RefCell::new(PlayerESP::new())),
            Rc::new(RefCell::new(SpectatorsList::new())),
            Rc::new(RefCell::new(BombInfo::new())),
            Rc::new(RefCell::new(TriggerBot::new(LocalCrosshair::new(
                cs2_offsets.offset_crosshair_id,
            )))),
            Rc::new(RefCell::new(AntiAimPunsh::new())),
        ],

        last_total_read_calls: 0,
        frame_read_calls: 0,

        settings: settings.clone(),
        settings_visible: false,
        settings_dirty: false,
        settings_ui: RefCell::new(SettingsUI::new(settings)),
        /* set the screen capture visibility at the beginning of the first update */
        settings_screen_capture_changed: AtomicBool::new(true),
        settings_render_debug_window_changed: AtomicBool::new(true),
    };
    let app = Rc::new(RefCell::new(app));

    cs2.add_metrics_record(
        obfstr!("controller-status"),
        &format!(
            "initialized, version: CHS-{}, git-hash: {}, win-build: {}",
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HASH"),
            build_info.dwBuildNumber
        ),
    );

    log::info!("{}", obfstr!("应用程序已初始化。正在生成叠加层..."));
    let mut update_fail_count = 0;
    let mut update_timeout: Option<(Instant, Duration)> = None;
    overlay.main_loop(
        {
            let app = app.clone();
            move |controller| {
                let mut app = app.borrow_mut();
                if let Err(err) = app.pre_update(controller) {
                    show_critical_error(&format!("{:#}", err));
                    false
                } else {
                    true
                }
            }
        },
        move |ui| {
            let mut app = app.borrow_mut();

            if let Some((timeout, target)) = &update_timeout {
                if timeout.elapsed() > *target {
                    update_timeout = None;
                } else {
                    /* Not updating. On timeout... */
                    return true;
                }
            }

            if let Err(err) = app.update(ui) {
                if update_fail_count >= 10 {
                    log::error!("出现 10 多个错误。等待 1 秒后再试。");
                    log::error!("最后一个错误: {:#}", err);

                    update_timeout = Some((Instant::now(), Duration::from_millis(1000)));
                    update_fail_count = 0;
                    return true;
                } else {
                    update_fail_count += 1;
                }
            }

            app.render(ui);
            true
        },
    )
}
