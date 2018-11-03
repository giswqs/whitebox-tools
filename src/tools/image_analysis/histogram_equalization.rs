/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: August 26, 2017
Last Modified: 13/10/2018
License: MIT

NOTES: 1. The tool should be updated to take multiple file inputs.
       2. Unlike the original Whitebox GAT tool that this is based on, 
          this tool will operate on RGB images in addition to greyscale images.
*/

use num_cpus;
use raster::*;
use std::env;
use std::f64;
use std::f64::consts::PI;
use std::io::{Error, ErrorKind};
use std::path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use tools::*;

pub struct HistogramEqualization {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl HistogramEqualization {
    pub fn new() -> HistogramEqualization {
        // public constructor
        let name = "HistogramEqualization".to_string();
        let toolbox = "Image Processing Tools/Image Enhancement".to_string();
        let description =
            "Performs a histogram equalization contrast enhancment on an image.".to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input File".to_owned(),
            flags: vec!["-i".to_owned(), "--input".to_owned()],
            description: "Input raster file.".to_owned(),
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
            name: "Number of Tones".to_owned(),
            flags: vec!["--num_tones".to_owned()],
            description: "Number of tones in the output image.".to_owned(),
            parameter_type: ParameterType::Integer,
            default_value: Some("256".to_owned()),
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
            ">>.*{0} -r={1} -v --wd=\"*path*to*data*\" -i=input.tif -o=output.tif --num_tones=1024",
            short_exe, name
        ).replace("*", &sep);

        HistogramEqualization {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for HistogramEqualization {
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
        let mut num_tones = 256f64;

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
            } else if vec[0].to_lowercase() == "-num_tones"
                || vec[0].to_lowercase() == "--num_tones"
            {
                if keyval {
                    num_tones = vec[1].to_string().parse::<f64>().unwrap();
                } else {
                    num_tones = args[i + 1].to_string().parse::<f64>().unwrap();
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

        if num_tones < 16f64 {
            println!("Warning: The output number of greytones must be at least 16. The value has been modified.");
            num_tones = 16f64;
        }
        let num_tones_less_one = num_tones - 1f64;

        if verbose {
            println!("Reading input data...")
        };
        let input = Arc::new(Raster::new(&input_file, "r")?);
        let rows = input.configs.rows as isize;
        let columns = input.configs.columns as isize;
        let nodata = input.configs.nodata;

        let is_rgb_image = if input.configs.data_type == DataType::RGB24
            || input.configs.data_type == DataType::RGBA32
            || input.configs.photometric_interp == PhotometricInterpretation::RGB
        {
            true
        } else {
            false
        };

        if input.configs.data_type == DataType::RGB48 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "This tool cannot be applied to 48-bit RGB colour-composite images.",
            ));
        }

        let start = Instant::now();

        if verbose {
            println!("Calculating clip values...")
        };

        // create the histogram
        let mut num_bins = 1024usize;
        let mut histo = vec![0f64; num_bins];
        let mut n = 0f64;
        let min_value: f64;
        let range: f64;
        let bin_size: f64;
        if !is_rgb_image {
            min_value = input.configs.minimum;
            range = input.configs.maximum - min_value;
            if range.round() as usize > num_bins {
                num_bins = range.round() as usize;
                histo = vec![0f64; num_bins];
            }
            bin_size = range / (num_bins - 1) as f64;
            let mut bin: usize;
            let mut value: f64;
            for row in 0..rows {
                for col in 0..columns {
                    value = input.get_value(row, col);
                    if value != nodata {
                        n += 1f64;
                        bin = ((value - min_value) / bin_size).floor() as usize;
                        histo[bin] += 1f64;
                    }
                }
                if verbose {
                    progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                    if progress != old_progress {
                        println!("Calculating histogram: {}%", progress);
                        old_progress = progress;
                    }
                }
            }
        } else {
            min_value = 0f64;
            range = 1f64;
            bin_size = range / (num_bins - 1) as f64;
            let mut value: f64;
            let mut v: f64;
            let mut bin: usize;
            for row in 0..rows {
                for col in 0..columns {
                    value = input.get_value(row, col);
                    if value != nodata {
                        v = value2i(value);
                        n += 1f64;
                        bin = (v * (num_bins - 1) as f64).floor() as usize;
                        histo[bin] += 1f64;
                    }
                }
                if verbose {
                    progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                    if progress != old_progress {
                        println!("Calculating histogram: {}%", progress);
                        old_progress = progress;
                    }
                }
            }
        }

        let mut cdf = vec![0f64; histo.len()];
        let min_nonempty_bin = histo[0];
        cdf[0] = histo[0];
        for j in 1..histo.len() {
            cdf[j] = cdf[j - 1] + histo[j];
        }

        let cdf = Arc::new(cdf); // wrap the cdf in an arc

        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for tid in 0..num_procs {
            let input = input.clone();
            let cdf = cdf.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let input_fn: Box<Fn(isize, isize) -> usize> = if !is_rgb_image {
                    Box::new(|row: isize, col: isize| -> usize {
                        let x = input.get_value(row, col);
                        ((x - min_value) / bin_size).floor() as usize
                    })
                } else {
                    Box::new(|row: isize, col: isize| -> usize {
                        let value = input.get_value(row, col);
                        let x = value2i(value);
                        ((x - min_value) / bin_size).floor() as usize
                    })
                };

                let output_fn: Box<Fn(isize, isize, f64) -> f64> = if !is_rgb_image {
                    Box::new(|_: isize, _: isize, value: f64| -> f64 { value })
                } else {
                    Box::new(|row: isize, col: isize, value: f64| -> f64 {
                        if value != nodata {
                            // convert the value into an rgb value based on modified hsi values.
                            let (h, s, _) = value2hsi(input.get_value(row, col));
                            let ret = hsi2value(h, s, value / num_tones_less_one);
                            return ret;
                        }
                        nodata
                    })
                };

                let num_cells_less_one = n - min_nonempty_bin; //n - 1f64;
                let mut z_in: f64;
                let mut z_out: f64;
                let mut bin: usize;
                for row in (0..rows).filter(|r| r % num_procs == tid) {
                    let mut data: Vec<f64> = vec![nodata; columns as usize];
                    for col in 0..columns {
                        z_in = input[(row, col)];
                        if z_in != nodata {
                            bin = input_fn(row, col);
                            z_out = ((cdf[bin] - min_nonempty_bin) / num_cells_less_one
                                * num_tones_less_one)
                                .round();
                            data[col as usize] = output_fn(row, col, z_out);
                        }
                    }
                    tx.send((row, data)).unwrap();
                }
            });
        }

        let mut output = Raster::initialize_using_file(&output_file, &input);
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

        let elapsed_time = get_formatted_elapsed_time(start);
        output.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output.add_metadata_entry(format!("Input file: {}", input_file));
        output.add_metadata_entry(format!("Number of tones: {}", num_tones));
        output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time));

        if verbose {
            println!("Saving data...")
        };
        let _ = match output.write() {
            Ok(_) => {
                if verbose {
                    println!("Output file written")
                }
            }
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

#[inline]
fn value2i(value: f64) -> f64 {
    let r = (value as u32 & 0xFF) as f64 / 255f64;
    let g = ((value as u32 >> 8) & 0xFF) as f64 / 255f64;
    let b = ((value as u32 >> 16) & 0xFF) as f64 / 255f64;

    (r + g + b) / 3f64
}

#[inline]
fn value2hsi(value: f64) -> (f64, f64, f64) {
    let r = (value as u32 & 0xFF) as f64 / 255f64;
    let g = ((value as u32 >> 8) & 0xFF) as f64 / 255f64;
    let b = ((value as u32 >> 16) & 0xFF) as f64 / 255f64;

    let i = (r + g + b) / 3f64;

    let rn = r / (r + g + b);
    let gn = g / (r + g + b);
    let bn = b / (r + g + b);

    let mut h = if rn != gn || rn != bn {
        ((0.5 * ((rn - gn) + (rn - bn))) / ((rn - gn) * (rn - gn) + (rn - bn) * (gn - bn)).sqrt())
            .acos()
    } else {
        0f64
    };
    if b > g {
        h = 2f64 * PI - h;
    }

    let s = 1f64 - 3f64 * rn.min(gn).min(bn);

    (h, s, i)
}

#[inline]
fn hsi2value(h: f64, s: f64, i: f64) -> f64 {
    let mut r: u32;
    let mut g: u32;
    let mut b: u32;

    let x = i * (1f64 - s);

    if h < 2f64 * PI / 3f64 {
        let y = i * (1f64 + (s * h.cos()) / ((PI / 3f64 - h).cos()));
        let z = 3f64 * i - (x + y);
        r = (y * 255f64).round() as u32;
        g = (z * 255f64).round() as u32;
        b = (x * 255f64).round() as u32;
    } else if h < 4f64 * PI / 3f64 {
        let h = h - 2f64 * PI / 3f64;
        let y = i * (1f64 + (s * h.cos()) / ((PI / 3f64 - h).cos()));
        let z = 3f64 * i - (x + y);
        r = (x * 255f64).round() as u32;
        g = (y * 255f64).round() as u32;
        b = (z * 255f64).round() as u32;
    } else {
        let h = h - 4f64 * PI / 3f64;
        let y = i * (1f64 + (s * h.cos()) / ((PI / 3f64 - h).cos()));
        let z = 3f64 * i - (x + y);
        r = (z * 255f64).round() as u32;
        g = (x * 255f64).round() as u32;
        b = (y * 255f64).round() as u32;
    }

    if r > 255u32 {
        r = 255u32;
    }
    if g > 255u32 {
        g = 255u32;
    }
    if b > 255u32 {
        b = 255u32;
    }

    ((255 << 24) | (b << 16) | (g << 8) | r) as f64
}
