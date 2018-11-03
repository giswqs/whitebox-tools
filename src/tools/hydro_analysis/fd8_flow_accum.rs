/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: June 26, 2017
Last Modified: 12/10/2018
License: MIT
*/

use num_cpus;
use raster::*;
use std::env;
use std::f64;
use std::io::{Error, ErrorKind};
use std::path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use structures::Array2D;
use tools::*;

pub struct FD8FlowAccumulation {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl FD8FlowAccumulation {
    pub fn new() -> FD8FlowAccumulation {
        // public constructor
        let name = "FD8FlowAccumulation".to_string();
        let toolbox = "Hydrological Analysis".to_string();
        let description =
            "Calculates an FD8 flow accumulation raster from an input DEM.".to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input DEM File".to_owned(),
            flags: vec!["-i".to_owned(), "--dem".to_owned()],
            description: "Input raster DEM file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output File".to_owned(),
            flags: vec!["-o".to_owned(), "--output".to_owned()],
            description: "Output raster file.".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter{
            name: "Output Type".to_owned(), 
            flags: vec!["--out_type".to_owned()], 
            description: "Output type; one of 'cells', 'specific contributing area' (default), and 'catchment area'.".to_owned(),
            parameter_type: ParameterType::OptionList(vec!["cells".to_owned(), "specific contributing area".to_owned(), "catchment area".to_owned()]),
            default_value: Some("specific contributing area".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter {
            name: "Exponent Parameter".to_owned(),
            flags: vec!["--exponent".to_owned()],
            description: "Optional exponent parameter; default is 1.1.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("1.1".to_owned()),
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Convergence Threshold (grid cells; blank for none)".to_owned(),
            flags: vec!["--threshold".to_owned()],
            description:
                "Optional convergence threshold parameter, in grid cells; default is inifinity."
                    .to_owned(),
            parameter_type: ParameterType::Float,
            default_value: None,
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Log-transform the output?".to_owned(),
            flags: vec!["--log".to_owned()],
            description: "Optional flag to request the output be log-transformed.".to_owned(),
            parameter_type: ParameterType::Boolean,
            default_value: None,
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Clip the upper tail by 1%?".to_owned(),
            flags: vec!["--clip".to_owned()],
            description: "Optional flag to request clipping the display max by 1%.".to_owned(),
            parameter_type: ParameterType::Boolean,
            default_value: None,
            optional: true,
        });

        let sep: String = path::MAIN_SEPARATOR.to_string();
        let p = format!("{}", env::current_dir().unwrap().display());
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut short_exe = e
            .replace(&p, "")
            .replace(".exe", "")
            .replace(".", "")
            .replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(">>.*{0} -r={1} -v --wd=\"*path*to*data*\" --dem=DEM.tif -o=output.tif --out_type='cells'
>>.*{0} -r={1} -v --wd=\"*path*to*data*\" --dem=DEM.tif -o=output.tif --out_type='catchment area' --exponent=1.5 --threshold=10000 --log --clip", short_exe, name).replace("*", &sep);

        FD8FlowAccumulation {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for FD8FlowAccumulation {
    fn get_source_file(&self) -> String {
        String::from(file!())
    }

    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        match serde_json::to_string(&self.parameters) {
            Ok(json_str) => return format!("{{\"parameters\":{}}}", json_str),
            Err(err) => return format!("{:?}", err),
        }
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn get_toolbox(&self) -> String {
        self.toolbox.clone()
    }

    fn run<'a>(
        &self,
        args: Vec<String>,
        working_directory: &'a str,
        verbose: bool,
    ) -> Result<(), Error> {
        let mut input_file = String::new();
        let mut output_file = String::new();
        let mut out_type = String::from("sca");
        let mut exponent = 1.1;
        let mut convergence_threshold = f64::INFINITY;
        let mut log_transform = false;
        let mut clip_max = false;

        if args.len() == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Tool run with no paramters.",
            ));
        }
        for i in 0..args.len() {
            let mut arg = args[i].replace("\"", "");
            arg = arg.replace("\'", "");
            let cmd = arg.split("="); // in case an equals sign was used
            let vec = cmd.collect::<Vec<&str>>();
            let mut keyval = false;
            if vec.len() > 1 {
                keyval = true;
            }
            if vec[0].to_lowercase() == "-i"
                || vec[0].to_lowercase() == "--input"
                || vec[0].to_lowercase() == "--dem"
            {
                if keyval {
                    input_file = vec[1].to_string();
                } else {
                    input_file = args[i + 1].to_string();
                }
            } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
                if keyval {
                    output_file = vec[1].to_string();
                } else {
                    output_file = args[i + 1].to_string();
                }
            } else if vec[0].to_lowercase() == "-out_type" || vec[0].to_lowercase() == "--out_type"
            {
                if keyval {
                    out_type = vec[1].to_lowercase();
                } else {
                    out_type = args[i + 1].to_lowercase();
                }
                if out_type.contains("specific") || out_type.contains("sca") {
                    out_type = String::from("sca");
                } else if out_type.contains("cells") {
                    out_type = String::from("cells");
                } else {
                    out_type = String::from("ca");
                }
            } else if vec[0].to_lowercase() == "-exponent" || vec[0].to_lowercase() == "--exponent"
            {
                if keyval {
                    exponent = vec[1].to_string().parse::<f64>().unwrap();
                } else {
                    exponent = args[i + 1].to_string().parse::<f64>().unwrap();
                }
            } else if vec[0].to_lowercase() == "-threshold"
                || vec[0].to_lowercase() == "--threshold"
            {
                if keyval {
                    convergence_threshold = vec[1].to_string().parse::<f64>().unwrap();
                } else {
                    convergence_threshold = args[i + 1].to_string().parse::<f64>().unwrap();
                }
            } else if vec[0].to_lowercase() == "-log" || vec[0].to_lowercase() == "--log" {
                log_transform = true;
            } else if vec[0].to_lowercase() == "-clip" || vec[0].to_lowercase() == "--clip" {
                clip_max = true;
            }
        }

        if verbose {
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
            println!("* Welcome to {} *", self.get_tool_name());
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
        }

        let sep: String = path::MAIN_SEPARATOR.to_string();

        let mut progress: usize;
        let mut old_progress: usize = 1;

        if !input_file.contains(&sep) && !input_file.contains("/") {
            input_file = format!("{}{}", working_directory, input_file);
        }
        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }

        if verbose {
            println!("Reading data...")
        };

        let input = Arc::new(Raster::new(&input_file, "r")?);

        let start = Instant::now();
        let rows = input.configs.rows as isize;
        let columns = input.configs.columns as isize;
        let num_cells = rows * columns;
        let nodata = input.configs.nodata;
        let cell_size_x = input.configs.resolution_x;
        let cell_size_y = input.configs.resolution_y;
        let diag_cell_size = (cell_size_x * cell_size_x + cell_size_y * cell_size_y).sqrt();

        // calculate the number of inflowing cells
        let mut num_inflowing: Array2D<i8> = Array2D::new(rows, columns, -1, -1)?;
        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for tid in 0..num_procs {
            let input = input.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let d_x = [1, 1, 1, 0, -1, -1, -1, 0];
                let d_y = [-1, 0, 1, 1, 1, 0, -1, -1];
                let mut z: f64;
                let mut count: i8;
                let mut interior_pit_found = false;
                for row in (0..rows).filter(|r| r % num_procs == tid) {
                    let mut data: Vec<i8> = vec![-1i8; columns as usize];
                    for col in 0..columns {
                        z = input[(row, col)];
                        if z != nodata {
                            count = 0i8;
                            for i in 0..8 {
                                if input[(row + d_y[i], col + d_x[i])] > z {
                                    count += 1;
                                }
                            }
                            data[col as usize] = count;
                            if count == 8 {
                                interior_pit_found = true;
                            }
                        }
                    }
                    tx.send((row, data, interior_pit_found)).unwrap();
                }
            });
        }

        let mut output = Raster::initialize_using_file(&output_file, &input);
        output.reinitialize_values(1.0);
        let mut stack = Vec::with_capacity((rows * columns) as usize);
        let mut num_solved_cells = 0;
        let mut interior_pit_found = false;
        for r in 0..rows {
            let (row, data, pit) = rx.recv().unwrap();
            num_inflowing.set_row_data(row, data);
            if pit {
                interior_pit_found = true;
            }
            for col in 0..columns {
                if num_inflowing[(row, col)] == 0i8 {
                    stack.push((row, col));
                } else if num_inflowing[(row, col)] == -1i8 {
                    num_solved_cells += 1;
                }
            }

            if verbose {
                progress = (100.0_f64 * r as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Num. inflowing neighbours: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let d_x = [1, 1, 1, 0, -1, -1, -1, 0];
        let d_y = [-1, 0, 1, 1, 1, 0, -1, -1];
        let (mut row, mut col): (isize, isize);
        let (mut row_n, mut col_n): (isize, isize);
        let (mut z, mut z_n): (f64, f64);
        let mut fa: f64;
        let grid_lengths = [
            diag_cell_size,
            cell_size_x,
            diag_cell_size,
            cell_size_y,
            diag_cell_size,
            cell_size_x,
            diag_cell_size,
            cell_size_y,
        ];
        let (mut max_slope, mut slope): (f64, f64);
        let mut dir: i8;

        while !stack.is_empty() {
            let cell = stack.pop().unwrap();
            row = cell.0;
            col = cell.1;
            z = input[(row, col)];
            fa = output[(row, col)];
            num_inflowing[(row, col)] = -1i8;

            let mut total_weights = 0.0;
            let mut weights: [f64; 8] = [0.0; 8];
            let mut downslope: [bool; 8] = [false; 8];
            if fa < convergence_threshold {
                for i in 0..8 {
                    row_n = row + d_y[i];
                    col_n = col + d_x[i];
                    z_n = input[(row_n, col_n)];
                    if z_n < z && z_n != nodata {
                        weights[i] = (z - z_n).powf(exponent);
                        total_weights += weights[i];
                        downslope[i] = true;
                    }
                }
            } else {
                // find the steepest downslope neighbour and give it all to them
                dir = 0i8;
                max_slope = f64::MIN;
                for i in 0..8 {
                    z_n = input[(row + d_y[i], col + d_x[i])];
                    if z_n != nodata {
                        slope = (z - z_n) / grid_lengths[i];
                        if slope > 0f64 {
                            downslope[i] = true;
                            if slope > max_slope {
                                max_slope = slope;
                                dir = i as i8;
                            }
                        }
                    }
                }
                if max_slope >= 0f64 {
                    weights[dir as usize] = 1.0;
                    total_weights = 1.0;
                }
            }

            if total_weights > 0.0 {
                for i in 0..8 {
                    if downslope[i] {
                        row_n = row + d_y[i];
                        col_n = col + d_x[i];
                        output.increment(row_n, col_n, fa * (weights[i] / total_weights));
                        num_inflowing.decrement(row_n, col_n, 1i8);
                        if num_inflowing[(row_n, col_n)] == 0i8 {
                            stack.push((row_n, col_n));
                        }
                    }
                }
            }

            if verbose {
                num_solved_cells += 1;
                progress = (100.0_f64 * num_solved_cells as f64 / (num_cells - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Flow accumulation: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let mut cell_area = cell_size_x * cell_size_y;
        let mut avg_cell_size = (cell_size_x + cell_size_y) / 2.0;
        if out_type == "cells" {
            cell_area = 1.0;
            avg_cell_size = 1.0;
        } else if out_type == "ca" {
            avg_cell_size = 1.0;
        }

        if log_transform {
            for row in 0..rows {
                for col in 0..columns {
                    if input[(row, col)] == nodata {
                        output[(row, col)] = nodata;
                    } else {
                        output[(row, col)] = (output[(row, col)] * cell_area / avg_cell_size).ln();
                    }
                }

                if verbose {
                    progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                    if progress != old_progress {
                        println!("Correcting values: {}%", progress);
                        old_progress = progress;
                    }
                }
            }
        } else {
            for row in 0..rows {
                for col in 0..columns {
                    if input[(row, col)] == nodata {
                        output[(row, col)] = nodata;
                    } else {
                        output[(row, col)] = output[(row, col)] * cell_area / avg_cell_size;
                    }
                }

                if verbose {
                    progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                    if progress != old_progress {
                        println!("Correcting values: {}%", progress);
                        old_progress = progress;
                    }
                }
            }
        }

        output.configs.palette = "blueyellow.plt".to_string();
        if clip_max {
            output.clip_display_max(1.0);
        }
        let elapsed_time = get_formatted_elapsed_time(start);
        output.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output.add_metadata_entry(format!("Input file: {}", input_file));
        output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time));

        if verbose {
            println!("Saving data...")
        };
        let _ = match output.write() {
            Ok(_) => if verbose {
                println!("Output file written")
            },
            Err(e) => return Err(e),
        };
        if verbose {
            println!(
                "{}",
                &format!("Elapsed Time (excluding I/O): {}", elapsed_time)
            );
        }
        if interior_pit_found {
            println!("**********************************************************************************");
            println!("WARNING: Interior pit cells were found within the input DEM. It is likely that the 
            DEM needs to be processed to remove topographic depressions and flats prior to
            running this tool.");
            println!("**********************************************************************************");
        }

        Ok(())
    }
}
