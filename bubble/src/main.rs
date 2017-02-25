extern crate rand;
extern crate chrono;
extern crate rustc_serialize;
extern crate docopt;

use rand::Rng;
use rand::distributions::{IndependentSample, Range, Normal};
use std::fs::File;
use std::io::Write;
use chrono::prelude::*;
use docopt::Docopt;

const USAGE: &'static str = "
Social Network Bubble Simulator.

Usage:
  bubble <population> <degree> <rewire> <consensus> <opposition>
";


#[derive(Debug, RustcDecodable)]
struct Args {
	arg_population: usize,
	arg_degree: usize,
	arg_rewire: f64,
	arg_consensus: f64,
	arg_opposition: f64
}


#[derive(Debug)]
struct Matrix<T> {
    size: usize,
    data: Vec<T>
}

impl<T> Matrix<T> where T: Default + Copy + std::fmt::Display + std::cmp::PartialEq {
    fn new(size: usize) -> Self {
        Matrix {
            size: size,
            data: vec![T::default(); size * size],
        }
    }

    fn wattz_strogatz(n: usize, k: usize, beta: f64, marker: T) -> Matrix<T> {
        let mut matrix: Matrix<T> = Matrix::new(n);

        // Construct a ring lattice.
        for row in 0..n {
            let half_k = ((k as f64)/2.0_f64) as usize; 
            let mut col = matrix.size() - half_k + row;
            for _ in 0..(k+1) {

                if col > matrix.size() - 1 {
                    col -= matrix.size();
                } 
                if col == row {
                    col += 1;
                    continue;
                }

                matrix.put(row, col, marker);
                col += 1;
            }

        }

        // Rewire with probability beta. Be sure to symmetically rewire.
        let mut rng = rand::thread_rng();
        for row in 0..n {
            for col in 0..row {
                let value = matrix.get(row, col);
                if value == marker && rng.next_f64() <= beta {
                    for new_col in 0..n {
                        if new_col == row {
                            continue;
                        }

                        if rng.next_f64() <= 1.0_f64/(n as f64) {
                            matrix.put(row, col, T::default());
                            matrix.put(col, row, T::default());
                            matrix.put(row, new_col, marker);
                            matrix.put(new_col, row, marker);
                            break;
                        }
                    }
                    
                }
            }
        }

        return matrix;

    }

    fn print(&self) {
        for row in 0..self.size() {
            for col in 0..self.size() {
                let value: T = self.get(row, col);
                print!("{} ", value);
            }
            print!("\n");
        }
    }

    fn size(&self) -> usize {
        self.size
    }

    fn index_for(&self, row: usize, col: usize) -> usize {
        row * self.size + col
    }

    fn get(&self, row: usize, col: usize) -> T {
        let index = self.index_for(row, col);
        self.data[index]
    }

    fn put(&mut self, row: usize, col: usize, value: T) {
        let index = self.index_for(row, col);
        self.data[index] = value;
        let index2 = self.index_for(col,row);
        self.data[index2] = value;
    }
}

fn main() {

    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.decode())
                            .unwrap_or_else(|e| e.exit());

    let population: usize = args.arg_population;
    let mut rng = rand::thread_rng();
	let max_time = 10000;


    let consensus = args.arg_consensus;
	let irreconsilable = args.arg_opposition;

    // Open the opinions file.
    let utc: DateTime<UTC> = UTC::now();
    let date_string = utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut opinion_file = File::create("simulation_".to_string() + &date_string + ".csv")
                            .expect("Unable to create file.");
    let mut network_file = File::create("network_".to_string() + &date_string + ".csv")
                            .expect("Unable to create file.");
    let mut metadata_file = File::create("metadata_".to_string() + &date_string + ".csv")
                            .expect("Unable to create file.");

    write!(metadata_file, "{},{},{},{},{}\n", args.arg_population,
	       args.arg_degree,	args.arg_rewire, args.arg_consensus, 
           args.arg_opposition);



    // Generate the network
    let mut social_network: Matrix<f64> =  Matrix::wattz_strogatz(population,
                                                                  args.arg_degree, 
                                                                  args.arg_rewire,
                                                                  0.5_f64);
   
	// Initilize opinions
	let opinion_distribution = Normal::new(50.0, 10.0);
	let mut opinions: Vec<f64> = Vec::new();
	for _ in 0..population {
		opinions.push(opinion_distribution.ind_sample(&mut rng).abs());
	}

    // Store the initial state of the matrix
    for sender in 0..population {
        for recipient in 0..sender {
            let weight = social_network.get(sender, recipient);
            if sender == recipient || weight == 0.0_f64 {
                continue
            }
            write!(network_file, "0, {}, {}, {}\n", sender, recipient, weight);
        }
    }


	// Simulation loop
	// Here are the rule, every tick, we'll randomly pick a vertex and send a
	// message to it's neighbors. The opinion of the message will reflect the
	// opinions of the sender. Upon receiving the message, alter the reciever's
	// opinion by some percent of the difference in opinion.
	for tick in 1..max_time {
		
		let sender = rng.gen_range(0, population);
		let message_distribution = Normal::new(opinions[sender], 10.0);
		let message = message_distribution.ind_sample(&mut rng); 

		for recipient in 0..population {
			if social_network.get(sender, recipient) <= 0.0 {
				continue
			}

			// Adjust opinions
            let opinion_change =  social_network.get(sender, recipient) * 
									  ((message - opinions[recipient])/ 100.0);


			// Adjust social standing due to message. We're going to split this
			// into three categories.
			//	1. Consensus: Within 25% of each other. Increase relationship.
			//  2. Challenged: Within 75% of each other. Do nothing.
			//  3. Irreconsilable: More that 75% different. Decrease relationship
			let difference = (message - opinions[recipient]).abs(); 
			if difference < consensus {
				let strength = social_network.get(sender, recipient);
				let mut new_strength = strength + (consensus - difference)/100.0;
				if new_strength > 1.0 {
					new_strength = 1.0
				}
              

                // Adjust opinion so that the person's opinion is more in line
                // with the message.
                if message < opinions[recipient] {
                    opinions[recipient] -=  opinion_change.abs();
                } else {
                    opinions[recipient] +=  opinion_change.abs();
                }


				social_network.put(sender, 
								   recipient, 
								   new_strength);

		    } else if difference > irreconsilable {
				let strength = social_network.get(sender, recipient);
				let mut new_strength = strength - (difference - irreconsilable)/100.0;
				if new_strength < 0.0 {
					new_strength = 0.0
				}
				social_network.put(sender, 
								   recipient, 
								   new_strength);


               // Adjust opinion so that the person's opinion moves away from
               // the message.
               
                if message < opinions[recipient] {
                    opinions[recipient] +=  opinion_change.abs();
                } else {
                    opinions[recipient] -=  opinion_change.abs();
                }


			}
            
            write!(network_file, "{}, {}, {}, {}\n", tick, sender, 
                   recipient, social_network.get(sender, recipient));

		}

		// Cleanup opinions to be within [0, 100]
		for index in 0..population {
			if opinions[index] < 0.0 {
				opinions[index] = 0.0;
			} else if opinions[index] > 100.0 {
				opinions[index] = 100.0;
			}
		}


		for index in 0..population {
            write!(opinion_file, "{}, {}, {}\n", tick, index, opinions[index]);
		}

	}
}
