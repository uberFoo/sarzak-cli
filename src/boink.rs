use sarzak::v2::domain::Domain;

pub fn boink_main(domain: Domain) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Boink",
        native_options,
        Box::new(|cc| Box::new(Boink::new(cc, domain))),
    )
}

struct Boink {
    label: String,
    value: f32,
    domain: Domain,
}

impl Boink {
    fn new(_cc: &eframe::CreationContext<'_>, domain: Domain) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Self {
            label: "Hello World!".to_owned(),
            value: 2.7,
            domain: domain,
        }
    }
}

impl eframe::App for Boink {
    /// Called by the frame work to save state before shutdown.
    // fn save(&mut self, storage: &mut dyn eframe::Storage) {}

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self {
            label,
            value,
            domain,
        } = self;
        let [width, height] = domain.extents();

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // ui.with_layout(
            //     egui::Layout::left_to_right(egui::Align::Center).with_cross_justify(true),
            //     |ui| {
            //         egui::ScrollArea::both()
            //             .max_width(*width as f32)
            //             .max_height(*height as f32)
            //             .id_source("paper")
            //             .show(ui, |ui| {
            //                 // ui.add_sized([*width as f32, *height as f32], egui::Button::new("First"));
            egui::Window::new(domain.name())
                .scroll2([true, true])
                .show(ctx, |ui| {
                    for i in 0..10 {
                        ui.push_id(i, |ui| {
                            egui::Window::new(domain.name()).scroll2([true, true]).show(
                                ctx,
                                |ui| {
                                    ui.label("Windows can be moved by dragging them.");
                                    ui.label("They are automatically sized based on contents.");
                                    ui.label("You can turn on resizing and scrolling if you like.");
                                    ui.label("You would normally choose either panels OR windows.");
                                },
                            );
                        });
                    }
                });
            //             });
            //     },
            // );
            egui::warn_if_debug_build(ui);
        });
    }
}
