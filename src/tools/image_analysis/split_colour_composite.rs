/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: July 15, 2017
Last Modified: 13/10/2018
License: MIT
*/

use num_cpus;
use raster::*;
use std::env;
use std::io::{Error, ErrorKind};
use std::path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use tools::*;

/// This tool splits an RGB colour composite image into seperate multispectral images.
pub struct SplitColourComposite {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl SplitColourComposite {
    /// Public constructor.
    pub fn new() -> SplitColourComposite {
        let name = "SplitColourComposite".to_string();
        let toolbox = "Image Processing Tools".to_string();
        let description =
            "This tool splits an RGB colour composite image into seperate multispectral images."
                .to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input Colour Composite Image File".to_owned(),
            flags: vec!["-i".to_owned(), "--input".to_owned()],
            description: "Input colour composite image file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output File".to_owned(),
            flags: vec!["-o".to_owned(), "--output".to_owned()],
            description: "Output raster file (suffixes of '_r', '_g', and '_b' will be appended)."
                .to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
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
        let usage = format!(
            ">>.*{} -r={} -v --wd=\"*path*to*data*\" -i=input.tif -o=output.tif",
            short_exe, name
        ).replace("*", &sep);

        SplitColourComposite {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for SplitColourComposite {
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
        let mut input_file = String::new();
        let mut output_file = String::new();
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
            if vec[0].to_lowercase() == "-i" || vec[0].to_lowercase() == "--input" {
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
        let nodata = input.configs.nodata;

        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for tid in 0..num_procs {
            let input = input.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut in_val: f64;
                let mut val: u32;
                let (mut red, mut green, mut blue): (u32, u32, u32);
                for row in (0..rows).filter(|r| r % num_procs == tid) {
                    let mut data_r = vec![nodata; columns as usize];
                    let mut data_g = vec![nodata; columns as usize];
                    let mut data_b = vec![nodata; columns as usize];
                    for col in 0..columns {
                        in_val = input.get_value(row, col);
                        if in_val != nodata {
                            val = in_val as u32;
                            red = val & 0xFF;
                            green = (val >> 8) & 0xFF;
                            blue = (val >> 16) & 0xFF;
                            data_r[col as usize] = red as f64;
                            data_g[col as usize] = green as f64;
                            data_b[col as usize] = blue as f64;
                        }
                    }
                    tx.send((row, data_r, data_g, data_b)).unwrap();
                }
            });
        }

        let mut output_r =
            Raster::initialize_using_file(&output_file.replace(".dep", "_red.dep"), &input);
        output_r.configs.photometric_interp = PhotometricInterpretation::Continuous;
        output_r.configs.data_type = DataType::F32;

        let mut output_g =
            Raster::initialize_using_file(&output_file.replace(".dep", "_green.dep"), &input);
        output_g.configs.photometric_interp = PhotometricInterpretation::Continuous;
        output_g.configs.data_type = DataType::F32;

        let mut output_b =
            Raster::initialize_using_file(&output_file.replace(".dep", "_blue.dep"), &input);
        output_b.configs.photometric_interp = PhotometricInterpretation::Continuous;
        output_b.configs.data_type = DataType::F32;

        for row in 0..rows {
            let data = rx.recv().unwrap();
            output_r.set_row_data(data.0, data.1);
            output_g.set_row_data(data.0, data.2);
            output_b.set_row_data(data.0, data.3);
            if verbose {
                progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Progress: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let elapsed_time = get_formatted_elapsed_time(start);

        output_r.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output_r.add_metadata_entry(format!("Input file: {}", input_file));
        output_r.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time));
        if verbose {
            println!("Saving red image...")
        };
        let _ = match output_r.write() {
            Ok(_) => if verbose {
                println!("Output file written")
            },
            Err(e) => return Err(e),
        };

        output_g.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output_g.add_metadata_entry(format!("Input file: {}", input_file));
        output_g.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time));
        if verbose {
            println!("Saving green image...")
        };
        let _ = match output_g.write() {
            Ok(_) => if verbose {
                println!("Output file written")
            },
            Err(e) => return Err(e),
        };

        output_b.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output_b.add_metadata_entry(format!("Input file: {}", input_file));
        output_b.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time));
        if verbose {
            println!("Saving blue image...")
        };
        let _ = match output_b.write() {
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

        Ok(())
    }
}
