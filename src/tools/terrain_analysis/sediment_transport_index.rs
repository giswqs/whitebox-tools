/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: July 2, 2017
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
use tools::*;

pub struct SedimentTransportIndex {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl SedimentTransportIndex {
    pub fn new() -> SedimentTransportIndex {
        // public constructor
        let name = "SedimentTransportIndex".to_string();
        let toolbox = "Geomorphometric Analysis".to_string();
        let description = "Calculates the sediment transport index.".to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input Specific Contributing Area (SCA) File".to_owned(),
            flags: vec!["--sca".to_owned()],
            description: "Input raster specific contributing area (SCA) file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Input Slope File".to_owned(),
            flags: vec!["--slope".to_owned()],
            description: "Input raster slope file.".to_owned(),
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

        parameters.push(ToolParameter {
            name: "Specific Contributing Area (SCA) Exponent".to_owned(),
            flags: vec!["--sca_exponent".to_owned()],
            description: "SCA exponent value.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("0.4".to_owned()),
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Slope Exponent".to_owned(),
            flags: vec!["--slope_exponent".to_owned()],
            description: "Slope exponent value.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("1.3".to_owned()),
            optional: false,
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
        let usage = format!(">>.*{0} -r={1} -v --wd=\"*path*to*data*\" --sca='flow_accum.tif' --slope='slope.tif' -o=output.tif --sca_exponent=0.5 --slope_exponent=1.0", short_exe, name).replace("*", &sep);

        SedimentTransportIndex {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for SedimentTransportIndex {
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
        let mut s = String::from("{\"parameters\": [");
        for i in 0..self.parameters.len() {
            if i < self.parameters.len() - 1 {
                s.push_str(&(self.parameters[i].to_string()));
                s.push_str(",");
            } else {
                s.push_str(&(self.parameters[i].to_string()));
            }
        }
        s.push_str("]}");
        s
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
        let mut sca_file = String::new();
        let mut slope_file = String::new();
        let mut output_file = String::new();
        let mut sca_exponent = 0.4;
        let mut slope_exponent = 1.3;

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
            if vec[0].to_lowercase() == "-sca" || vec[0].to_lowercase() == "--sca" {
                if keyval {
                    sca_file = vec[1].to_string();
                } else {
                    sca_file = args[i + 1].to_string();
                }
            } else if vec[0].to_lowercase() == "-slope" || vec[0].to_lowercase() == "--slope" {
                if keyval {
                    slope_file = vec[1].to_string();
                } else {
                    slope_file = args[i + 1].to_string();
                }
            } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
                if keyval {
                    output_file = vec[1].to_string();
                } else {
                    output_file = args[i + 1].to_string();
                }
            } else if vec[0].to_lowercase() == "-sca_exponent"
                || vec[0].to_lowercase() == "--sca_exponent"
            {
                if keyval {
                    sca_exponent = vec[1].to_string().parse::<f64>().unwrap();
                } else {
                    sca_exponent = args[i + 1].to_string().parse::<f64>().unwrap();
                }
            } else if vec[0].to_lowercase() == "-slope_exponent"
                || vec[0].to_lowercase() == "--slope_exponent"
            {
                if keyval {
                    slope_exponent = vec[1].to_string().parse::<f64>().unwrap();
                } else {
                    slope_exponent = args[i + 1].to_string().parse::<f64>().unwrap();
                }
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

        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }
        if !sca_file.contains(&sep) && !sca_file.contains("/") {
            sca_file = format!("{}{}", working_directory, sca_file);
        }
        if !slope_file.contains(&sep) && !slope_file.contains("/") {
            slope_file = format!("{}{}", working_directory, slope_file);
        }

        if verbose {
            println!("Reading data...")
        };
        let sca = Arc::new(Raster::new(&sca_file, "r")?);
        let slope = Arc::new(Raster::new(&slope_file, "r")?);

        let start = Instant::now();
        let rows = sca.configs.rows as isize;
        let columns = sca.configs.columns as isize;
        let sca_nodata = sca.configs.nodata;
        let slope_nodata = slope.configs.nodata;

        // make sure the input files have the same size
        if sca.configs.rows != slope.configs.rows || sca.configs.columns != slope.configs.columns {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "The input files must have the same number of rows and columns and spatial extent.",
            ));
        }

        // calculate the number of downslope cells
        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for tid in 0..num_procs {
            let sca = sca.clone();
            let slope = slope.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut sca_val: f64;
                let mut slope_val: f64;
                for row in (0..rows).filter(|r| r % num_procs == tid) {
                    let mut data: Vec<f64> = vec![sca_nodata; columns as usize];
                    for col in 0..columns {
                        sca_val = sca[(row, col)];
                        slope_val = slope[(row, col)];
                        if sca_val != sca_nodata && slope_val != slope_nodata {
                            data[col as usize] = (sca_exponent + 1f64)
                                * (sca_val / 22.13).powf(sca_exponent)
                                * (((slope_val.to_radians()) / 0.0896).sin()).powf(slope_exponent);
                        }
                    }
                    tx.send((row, data)).unwrap();
                }
            });
        }

        let mut output = Raster::initialize_using_file(&output_file, &sca);
        for r in 0..rows {
            let (row, data) = rx.recv().unwrap();
            output.set_row_data(row, data);

            if verbose {
                progress = (100.0_f64 * r as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Progress: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let elapsed_time = get_formatted_elapsed_time(start);;
        output.configs.data_type = DataType::F32;
        output.configs.palette = "grey.plt".to_string();
        output.configs.photometric_interp = PhotometricInterpretation::Continuous;
        output.clip_display_min_max(1.0);
        output.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output.add_metadata_entry(format!("SCA raster: {}", sca_file));
        output.add_metadata_entry(format!("Slope raster: {}", slope_file));
        output.add_metadata_entry(format!("SCA exponent: {}", sca_exponent));
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

        if sca.configs.maximum < 100.0 {
            println!("WARNING: The input SCA data layer contained only low values. It is likely that it has been
            log-transformed. This tool requires non-transformed SCA as an input.")
        }
        if verbose {
            println!(
                "{}",
                &format!("Elapsed Time (excluding I/O): {}", elapsed_time)
            );
        }

        Ok(())
    }
}
