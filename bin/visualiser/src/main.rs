use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::time::Instant;
use common::config::load_config;
use common::dashboard::DashboardBuffer;
use common::{ActuatorType, ActuatorStatus};
use async_impl;
use std::env;

struct DashboardApp {
    dashboard_buffer: DashboardBuffer,
    experiment_running: bool,
    start_time: Option<Instant>,
    config_path: String,
    mode: String,
    stats: SystemStats,
}

#[derive(Default)]
struct SystemStats {
    total_cycles: u64,
    missed_deadlines: u64,
    avg_processing_ns: f64,
    avg_latency_ns: f64,
    max_lateness_ns: i64,
    sensor_count: u64,
    actuator_counts: std::collections::HashMap<ActuatorType, u64>,
    emergency_count: u64,
}

impl DashboardApp {
    fn new(config_path: String, mode: String) -> Self {
        Self {
            dashboard_buffer: DashboardBuffer::new(1000),
            experiment_running: false,
            start_time: None,
            config_path,
            mode,
            stats: SystemStats::default(),
        }
    }

    fn update_stats(&mut self) {
        let data = self.dashboard_buffer.get_all();
        self.stats = SystemStats::default();
        
        for item in &data {
            if let Some(metrics) = &item.metrics {
                self.stats.total_cycles += 1;
                if !metrics.deadline_met {
                    self.stats.missed_deadlines += 1;
                }
                self.stats.avg_processing_ns += metrics.processing_time_ns as f64;
                self.stats.avg_latency_ns += metrics.total_latency_ns as f64;
                if metrics.lateness_ns > self.stats.max_lateness_ns {
                    self.stats.max_lateness_ns = metrics.lateness_ns;
                }
            }
            
            if item.sensor_data.is_some() {
                self.stats.sensor_count += 1;
            }
            
            if let Some((actuator_type, feedback)) = &item.actuator_feedback {
                *self.stats.actuator_counts.entry(*actuator_type).or_insert(0) += 1;
                if matches!(feedback.status, ActuatorStatus::Emergency) {
                    self.stats.emergency_count += 1;
                }
            }
        }
        
        if self.stats.total_cycles > 0 {
            self.stats.avg_processing_ns /= self.stats.total_cycles as f64;
            self.stats.avg_latency_ns /= self.stats.total_cycles as f64;
        }
    }
}

impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_stats();
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Header
                    ui.heading("🎛️ Real-Time Sensor-Actuator Dashboard");
                    ui.add_space(10.0);
                    
                    // Control Panel
                    egui::Frame::group(ui.style())
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let start_btn = ui.add_enabled(
                                    !self.experiment_running,
                                    egui::Button::new("▶ Start Experiment")
                                );
                                if start_btn.clicked() {
                                    self.start_experiment();
                                }
                                
                                let stop_btn = ui.add_enabled(
                                    self.experiment_running,
                                    egui::Button::new("⏹ Stop Experiment")
                                );
                                if stop_btn.clicked() {
                                    self.stop_experiment();
                                }
                                
                                if ui.button("🗑 Clear Data").clicked() {
                                    self.dashboard_buffer.clear();
                                    self.stats = SystemStats::default();
                                }
                                
                                ui.separator();
                                
                                let status_text = if self.experiment_running { 
                                    egui::RichText::new("🟢 Running").color(egui::Color32::GREEN)
                                } else { 
                                    egui::RichText::new("🔴 Stopped").color(egui::Color32::RED)
                                };
                                ui.label(status_text);
                                
                                if let Some(start) = self.start_time {
                                    let elapsed = start.elapsed().as_secs();
                                    ui.label(format!("⏱ {}s", elapsed));
                                }
                            });
                        });
                    
                    ui.add_space(10.0);
                    
                    // Statistics Row
                    ui.horizontal(|ui| {
                        // System Statistics
                        egui::Frame::group(ui.style())
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_width(350.0);
                                ui.strong("📊 System Statistics");
                                ui.separator();
                                
                                egui::Grid::new("stats_grid")
                                    .num_columns(2)
                                    .spacing([40.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.label("Total Cycles:");
                                        ui.label(format!("{}", self.stats.total_cycles));
                                        ui.end_row();
                                        
                                        ui.label("Missed Deadlines:");
                                        let miss_pct = if self.stats.total_cycles > 0 {
                                            (self.stats.missed_deadlines as f64 / self.stats.total_cycles as f64) * 100.0
                                        } else { 0.0 };
                                        ui.label(format!("{} ({:.2}%)", self.stats.missed_deadlines, miss_pct));
                                        ui.end_row();
                                        
                                        ui.label("Avg Processing:");
                                        ui.label(format!("{:.2} μs", self.stats.avg_processing_ns / 1000.0));
                                        ui.end_row();
                                        
                                        ui.label("Avg Latency:");
                                        ui.label(format!("{:.2} μs", self.stats.avg_latency_ns / 1000.0));
                                        ui.end_row();
                                        
                                        ui.label("Max Lateness:");
                                        ui.label(format!("{} ns", self.stats.max_lateness_ns));
                                        ui.end_row();
                                        
                                        ui.label("Sensor Readings:");
                                        ui.label(format!("{}", self.stats.sensor_count));
                                        ui.end_row();
                                        
                                        ui.label("Emergency Events:");
                                        ui.label(format!("{}", self.stats.emergency_count));
                                        ui.end_row();
                                    });
                            });
                        
                        // Actuator Statistics
                        egui::Frame::group(ui.style())
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_width(250.0);
                                ui.strong("⚙️ Actuator Statistics");
                                ui.separator();
                                
                                if self.stats.actuator_counts.is_empty() {
                                    ui.label("No actuator data yet");
                                } else {
                                    egui::Grid::new("actuator_stats_grid")
                                        .num_columns(2)
                                        .spacing([20.0, 4.0])
                                        .striped(true)
                                        .show(ui, |ui| {
                                            for (actuator, count) in &self.stats.actuator_counts {
                                                ui.label(format!("{:?}:", actuator));
                                                ui.label(format!("{} cycles", count));
                                                ui.end_row();
                                            }
                                        });
                                }
                            });
                    });
                    
                    ui.add_space(10.0);
                    
                    // Recent Data Tables Row
                    ui.horizontal_top(|ui| {
                        let recent_data = self.dashboard_buffer.get_recent(50);
                        
                        // Sensor Readings
                        egui::Frame::group(ui.style())
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_width(450.0);
                                ui.strong("📡 Recent Sensor Readings");
                                ui.separator();
                                
                                egui::ScrollArea::vertical()
                                    .id_source("sensor_scroll")
                                    .max_height(180.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("sensor_grid")
                                            .num_columns(4)
                                            .spacing([8.0, 4.0])
                                            .striped(true)
                                            .show(ui, |ui| {
                                                ui.strong("Force");
                                                ui.strong("Position");
                                                ui.strong("Temp");
                                                ui.strong("Time");
                                                ui.end_row();
                                                
                                                for item in recent_data.iter().rev().take(15) {
                                                    if let Some(sensor) = &item.sensor_data {
                                                        ui.label(format!("{:.2}", sensor.force));
                                                        ui.label(format!("{:.2}", sensor.position));
                                                        ui.label(format!("{:.2}", sensor.temperature));
                                                        ui.label(format!("{}", sensor.timestamp / 1_000_000));
                                                        ui.end_row();
                                                    }
                                                }
                                            });
                                    });
                            });
                        
                        // Actuator Feedback
                        egui::Frame::group(ui.style())
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_width(450.0);
                                ui.strong("🔧 Recent Actuator Feedback");
                                ui.separator();
                                
                                egui::ScrollArea::vertical()
                                    .id_source("actuator_scroll")
                                    .max_height(180.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("actuator_grid")
                                            .num_columns(4)
                                            .spacing([8.0, 4.0])
                                            .striped(true)
                                            .show(ui, |ui| {
                                                ui.strong("Actuator");
                                                ui.strong("Status");
                                                ui.strong("Control");
                                                ui.strong("Error");
                                                ui.end_row();
                                                
                                                for item in recent_data.iter().rev().take(15) {
                                                    if let Some((act_type, feedback)) = &item.actuator_feedback {
                                                        ui.label(format!("{:?}", act_type));
                                                        ui.label(format!("{:?}", feedback.status));
                                                        ui.label(format!("{:.2}", feedback.control_output));
                                                        ui.label(format!("{:.2}", feedback.error));
                                                        ui.end_row();
                                                    }
                                                }
                                            });
                                    });
                            });
                    });
                    
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    // Sensor Data Graphs
                    ui.heading("📈 Real-Time Sensor Data");
                    ui.add_space(5.0);
                    
                    let recent_data = self.dashboard_buffer.get_recent(50);
                    
                    ui.columns(3, |columns| {
                        // Force
                        columns[0].group(|ui| {
                            ui.strong("Force Values");
                            let force_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| item.sensor_data.map(|s| [i as f64, s.force]))
                                .collect();

                            if !force_data.is_empty() {
                                Plot::new("force_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(force_data))
                                            .color(egui::Color32::from_rgb(52, 152, 219)));
                                    });
                            }
                        });
                        
                        // Position
                        columns[1].group(|ui| {
                            ui.strong("Position Values");
                            let position_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| item.sensor_data.map(|s| [i as f64, s.position]))
                                .collect();

                            if !position_data.is_empty() {
                                Plot::new("position_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(position_data))
                                            .color(egui::Color32::from_rgb(46, 204, 113)));
                                    });
                            }
                        });
                        
                        // Temperature
                        columns[2].group(|ui| {
                            ui.strong("Temperature Values");
                            let temp_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| item.sensor_data.map(|s| [i as f64, s.temperature]))
                                .collect();

                            if !temp_data.is_empty() {
                                Plot::new("temperature_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(temp_data))
                                            .color(egui::Color32::from_rgb(231, 76, 60)));
                                    });
                            }
                        });
                    });
                    
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    // Performance Metrics Graphs
                    ui.heading("⚡ Performance Metrics");
                    ui.add_space(5.0);
                    
                    ui.columns(2, |columns| {
                        // Processing Time
                        columns[0].group(|ui| {
                            ui.strong("Processing Time (μs)");
                            let processing_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| {
                                    item.metrics.as_ref().map(|m| [i as f64, m.processing_time_ns as f64 / 1000.0])
                                })
                                .collect();

                            if !processing_data.is_empty() {
                                Plot::new("processing_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(processing_data))
                                            .color(egui::Color32::from_rgb(230, 126, 34)));
                                    });
                            }
                        });
                        
                        // Latency
                        columns[1].group(|ui| {
                            ui.strong("Total Latency (μs)");
                            let latency_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| {
                                    item.metrics.as_ref().and_then(|m| {
                                        if m.total_latency_ns > 0 {
                                            Some([i as f64, m.total_latency_ns as f64 / 1000.0])
                                        } else { None }
                                    })
                                })
                                .collect();

                            if !latency_data.is_empty() {
                                Plot::new("latency_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(latency_data))
                                            .color(egui::Color32::from_rgb(155, 89, 182)));
                                    });
                            }
                        });
                    });
                    
                    ui.add_space(10.0);
                    
                    ui.columns(2, |columns| {
                        // Deadline Compliance
                        columns[0].group(|ui| {
                            ui.strong("Deadline Compliance");
                            let deadline_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| {
                                    item.metrics.as_ref().map(|m| [i as f64, if m.deadline_met { 1.0 } else { 0.0 }])
                                })
                                .collect();

                            if !deadline_data.is_empty() {
                                Plot::new("deadline_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(deadline_data))
                                            .color(egui::Color32::from_rgb(39, 174, 96)));
                                    });
                            }
                        });
                        
                        // Lateness
                        columns[1].group(|ui| {
                            ui.strong("Lateness (ns)");
                            let lateness_data: Vec<[f64; 2]> = recent_data
                                .iter()
                                .rev()
                                .enumerate()
                                .filter_map(|(i, item)| {
                                    item.metrics.as_ref().map(|m| [i as f64, m.lateness_ns as f64])
                                })
                                .collect();

                            if !lateness_data.is_empty() {
                                Plot::new("lateness_plot")
                                    .height(150.0)
                                    .show_axes([false, true])
                                    .allow_scroll(false)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(Line::new(PlotPoints::new(lateness_data))
                                            .color(egui::Color32::from_rgb(192, 57, 43)));
                                    });
                            }
                        });
                    });
                    
                    ui.add_space(20.0);
                });
        });
    }
}

impl DashboardApp {
    fn start_experiment(&mut self) {
        self.experiment_running = true;
        self.start_time = Some(Instant::now());
        self.dashboard_buffer.clear();
        
        let buffer = self.dashboard_buffer.clone();
        let config_path = self.config_path.clone();
        let _mode = self.mode.clone();
        
        std::thread::spawn(move || {
            let config = load_config(&config_path).expect("Failed to load config");
            
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _recorder = async_impl::run_experiment_with_dashboard(config, Some(buffer)).await;
            });
        });
    }
    
    fn stop_experiment(&mut self) {
        self.experiment_running = false;
    }
}

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1).map(|s| s.clone()).unwrap_or_else(|| "configs/experiment_baseline.toml".to_string());
    let mode = args.get(2).map(|s| s.clone()).unwrap_or_else(|| "async".to_string());
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Real-Time Sensor-Actuator Dashboard"),
        ..Default::default()
    };
    
    let mode_clone = mode.clone();
    let config_path_clone = config_path.clone();
    eframe::run_native(
        "Real-Time Dashboard",
        options,
        Box::new(move |_cc| Box::new(DashboardApp::new(config_path_clone.clone(), mode_clone.clone()))),
    )
}