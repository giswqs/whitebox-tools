/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: June 22, 2017
Last Modified: 12/10/2018
License: MIT
*/

use raster::*;
use std::env;
use std::f64;
use std::io::{Error, ErrorKind};
use std::path;
use structures::Array2D;
use tools::*;
use vector::*;

pub struct Watershed {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl Watershed {
    pub fn new() -> Watershed {
        // public constructor
        let name = "Watershed".to_string();
        let toolbox = "Hydrological Analysis".to_string();
        let description =
            "Identifies the watershed, or drainage basin, draining to a set of target cells."
                .to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input D8 Pointer File".to_owned(),
            flags: vec!["--d8_pntr".to_owned()],
            description: "Input D8 pointer raster file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Input Pour Points (Outlet) File".to_owned(),
            flags: vec!["--pour_pts".to_owned()],
            description: "Input vector pour points (outlet) file.".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Vector(
                VectorGeometryType::Point,
            )),
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
            name: "Does the pointer file use the ESRI pointer scheme?".to_owned(),
            flags: vec!["--esri_pntr".to_owned()],
            description: "D8 pointer uses the ESRI style scheme.".to_owned(),
            parameter_type: ParameterType::Boolean,
            default_value: Some("false".to_owned()),
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
        let usage = format!(">>.*{0} -r={1} -v --wd=\"*path*to*data*\" --d8_pntr='d8pntr.tif' --pour_pts='pour_pts.shp' -o='output.tif'", short_exe, name).replace("*", &sep);

        Watershed {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for Watershed {
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
        let mut d8_file = String::new();
        let mut pourpts_file = String::new();
        let mut output_file = String::new();
        let mut esri_style = false;

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
            let flag_val = vec[0].to_lowercase().replace("--", "-");
            if flag_val == "-d8_pntr" {
                d8_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-pour_pts" {
                pourpts_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-o" || flag_val == "-output" {
                output_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-esri_pntr" || flag_val == "-esri_style" {
                esri_style = true;
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

        if !d8_file.contains(&sep) && !d8_file.contains("/") {
            d8_file = format!("{}{}", working_directory, d8_file);
        }
        if !pourpts_file.contains(&sep) && !pourpts_file.contains("/") {
            pourpts_file = format!("{}{}", working_directory, pourpts_file);
        }
        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }

        if verbose {
            println!("Reading data...")
        };

        let pntr = Raster::new(&d8_file, "r")?;

        // let pourpts = Raster::new(&pourpts_file, "r")?;
        let pourpts = Shapefile::read(&pourpts_file)?;

        // make sure the input vector file is of points type
        if pourpts.header.shape_type.base_shape_type() != ShapeType::Point {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "The input vector data must be of point base shape type.",
            ));
        }

        let start = Instant::now();

        let rows = pntr.configs.rows as isize;
        let columns = pntr.configs.columns as isize;
        let nodata = -32768f64; //pour_pts.configs.nodata;
        let pntr_nodata = pntr.configs.nodata;
        // let palette = pourpts.configs.palette.clone();

        // // make sure the input files have the same size
        // if pourpts.configs.rows != pntr.configs.rows || pourpts.configs.columns != pntr.configs.columns {
        //     return Err(Error::new(ErrorKind::InvalidInput,
        //                         "The input files must have the same number of rows and columns and spatial extent."));
        // }

        let dx = [1, 1, 1, 0, -1, -1, -1, 0];
        let dy = [-1, 0, 1, 1, 1, 0, -1, -1];

        let mut flow_dir: Array2D<i8> = Array2D::new(rows, columns, -2, -2)?;
        let mut output = Raster::initialize_using_file(&output_file, &pntr);
        output.configs.nodata = nodata;
        output.configs.data_type = DataType::I16;
        output.configs.photometric_interp = PhotometricInterpretation::Categorical;
        output.configs.palette = "qual.pal".to_string(); //palette;
        let low_value = f64::MIN;
        output.reinitialize_values(low_value);

        for record_num in 0..pourpts.num_records {
            let record = pourpts.get_record(record_num);
            let row = pntr.get_row_from_y(record.points[0].y);
            let col = pntr.get_column_from_x(record.points[0].x);
            output.set_value(row, col, (record_num + 1) as f64);

            if verbose {
                progress =
                    (100.0_f64 * record_num as f64 / (pourpts.num_records - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Locating pour points: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        // Create a mapping from the pointer values to cells offsets.
        // This may seem wasteful, using only 8 of 129 values in the array,
        // but the mapping method is far faster than calculating z.ln() / ln(2.0).
        // It's also a good way of allowing for different point styles.
        let mut pntr_matches: [i8; 129] = [0i8; 129];
        if !esri_style {
            // This maps Whitebox-style D8 pointer values
            // onto the cell offsets in dx and dy.
            pntr_matches[1] = 0i8;
            pntr_matches[2] = 1i8;
            pntr_matches[4] = 2i8;
            pntr_matches[8] = 3i8;
            pntr_matches[16] = 4i8;
            pntr_matches[32] = 5i8;
            pntr_matches[64] = 6i8;
            pntr_matches[128] = 7i8;
        } else {
            // This maps Esri-style D8 pointer values
            // onto the cell offsets in dx and dy.
            pntr_matches[1] = 1i8;
            pntr_matches[2] = 2i8;
            pntr_matches[4] = 3i8;
            pntr_matches[8] = 4i8;
            pntr_matches[16] = 5i8;
            pntr_matches[32] = 6i8;
            pntr_matches[64] = 7i8;
            pntr_matches[128] = 0i8;
        }

        let mut z: f64;
        for row in 0..rows {
            for col in 0..columns {
                z = pntr[(row, col)];
                if z != pntr_nodata {
                    if z > 0.0 {
                        flow_dir[(row, col)] = pntr_matches[z as usize];
                    } else {
                        flow_dir[(row, col)] = -1i8;
                    }
                } else {
                    output[(row, col)] = nodata;
                }
                // z = pourpts[(row, col)];
                // if z != nodata && z > 0.0 {
                //     output[(row, col)] = z;
                // }
            }
            if verbose {
                progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Initializing: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        let mut flag: bool;
        let (mut x, mut y): (isize, isize);
        let mut dir: i8;
        let mut outlet_id: f64;
        for row in 0..rows {
            for col in 0..columns {
                if output[(row, col)] == low_value {
                    flag = false;
                    x = col;
                    y = row;
                    outlet_id = nodata;
                    while !flag {
                        // find its downslope neighbour
                        dir = flow_dir[(y, x)];
                        if dir >= 0 {
                            // move x and y accordingly
                            x += dx[dir as usize];
                            y += dy[dir as usize];

                            // if the new cell already has a value in the output, use that as the outletID
                            z = output[(y, x)];
                            if z != low_value {
                                outlet_id = z;
                                flag = true;
                            }
                        } else {
                            flag = true;
                        }
                    }

                    flag = false;
                    x = col;
                    y = row;
                    output[(y, x)] = outlet_id;
                    while !flag {
                        // find its downslope neighbour
                        dir = flow_dir[(y, x)];
                        if dir >= 0 {
                            // move x and y accordingly
                            x += dx[dir as usize];
                            y += dy[dir as usize];

                            // if the new cell already has a value in the output, use that as the outletID
                            if output[(y, x)] != low_value {
                                flag = true;
                            }
                        } else {
                            flag = true;
                        }
                        output[(y, x)] = outlet_id;
                    }
                }
            }
            if verbose {
                progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
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
        output.add_metadata_entry(format!("D8 pointer file: {}", d8_file));
        output.add_metadata_entry(format!("Pour-points file: {}", pourpts_file));
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

        Ok(())
    }
}
