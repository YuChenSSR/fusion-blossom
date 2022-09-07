//! Parallel Primal Module
//! 
//! A parallel implementation of the primal module, by calling functions provided by the serial primal module
//!

use super::util::*;
use serde::{Serialize, Deserialize};
// use crate::derivative::Derivative;
use super::primal_module::*;
use super::primal_module_serial::*;
use super::visualize::*;
use super::dual_module::*;
use std::sync::Arc;


pub struct PrimalModuleParallel {
    /// the basic wrapped serial modules at the beginning, afterwards the fused units are appended after them
    pub units: Vec<ArcRwLock<PrimalModuleParallelUnit>>,
    /// thread pool used to execute async functions in parallel
    pub thread_pool: rayon::ThreadPool,
}

pub struct PrimalModuleParallelUnit {
    /// the index
    pub unit_index: usize,
    /// the owned serial primal module
    pub serial_module: PrimalModuleSerial,
}

pub type PrimalModuleParallelUnitPtr = ArcRwLock<PrimalModuleParallelUnit>;
pub type PrimalModuleParallelUnitWeak = WeakRwLock<PrimalModuleParallelUnit>;

impl std::fmt::Debug for PrimalModuleParallelUnitPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let unit = self.read_recursive();
        write!(f, "{}", unit.unit_index)
    }
}

impl std::fmt::Debug for PrimalModuleParallelUnitWeak {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.upgrade_force().fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrimalModuleParallelConfig {
    /// enable async execution of dual operations; only used when calling top-level operations, not used in individual units
    #[serde(default = "primal_module_parallel_default_configs::thread_pool_size")]
    pub thread_pool_size: usize,
}

impl Default for PrimalModuleParallelConfig {
    fn default() -> Self { serde_json::from_value(json!({})).unwrap() }
}

pub mod primal_module_parallel_default_configs {
    // pub fn thread_pool_size() -> usize { 0 }  // by default to the number of CPU cores
    pub fn thread_pool_size() -> usize { 1 }  // debug: use a single core
}

impl PrimalModuleParallel {

    /// recommended way to create a new instance, given a customized configuration
    pub fn new_config(initializer: &SolverInitializer, partition_info: Arc<PartitionInfo>, config: PrimalModuleParallelConfig) -> Self {
        unimplemented!()
    }

}

impl PrimalModuleImpl for PrimalModuleParallel {

    fn new(initializer: &SolverInitializer) -> Self {
        Self::new_config(initializer, PartitionConfig::default(initializer).into_info(initializer), PrimalModuleParallelConfig::default())
    }

    fn clear(&mut self) {
        unimplemented!()
    }
    
    fn load(&mut self, interface: &DualModuleInterface) {
        unimplemented!()
    }

    fn resolve<D: DualModuleImpl>(&mut self, mut group_max_update_length: GroupMaxUpdateLength, interface: &mut DualModuleInterface, dual_module: &mut D) {
        unimplemented!()
    }

    fn intermediate_matching<D: DualModuleImpl>(&mut self, _interface: &mut DualModuleInterface, _dual_module: &mut D) -> IntermediateMatching {
        unimplemented!()
    }

}

impl FusionVisualizer for PrimalModuleParallel {
    fn snapshot(&self, abbrev: bool) -> serde_json::Value {
        unimplemented!()
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;
    use super::super::example::*;
    use super::super::dual_module_parallel::*;
    use super::super::dual_module_serial::*;
    use std::sync::Arc;

    pub fn primal_module_parallel_basic_standard_syndrome_optional_viz<F>(mut code: impl ExampleCode, visualize_filename: Option<String>
            , mut syndrome_vertices: Vec<VertexIndex>, final_dual: Weight, partition_func: F, reordered_vertices: Option<Vec<VertexIndex>>)
            -> (DualModuleInterface, PrimalModuleParallel, DualModuleParallel<DualModuleSerial>) where F: Fn(&SolverInitializer, &mut PartitionConfig) {
        println!("{syndrome_vertices:?}");
        if let Some(reordered_vertices) = &reordered_vertices {
            code.reorder_vertices(reordered_vertices);
            syndrome_vertices = code.translated_syndrome_to_reordered(reordered_vertices, syndrome_vertices);
        }
        let mut visualizer = match visualize_filename.as_ref() {
            Some(visualize_filename) => {
                let mut visualizer = Visualizer::new(Some(visualize_data_folder() + visualize_filename.as_str())).unwrap();
                visualizer.set_positions(code.get_positions(), true);  // automatic center all nodes
                print_visualize_link(&visualize_filename);
                Some(visualizer)
            }, None => None
        };
        // create dual module
        let initializer = code.get_initializer();
        let mut partition_config = PartitionConfig::default(&initializer);
        partition_func(&initializer, &mut partition_config);
        println!("partition_config: {partition_config:?}");
        let partition_info = partition_config.into_info(&initializer);
        let mut dual_module = DualModuleParallel::new_config(&initializer, Arc::clone(&partition_info), DualModuleParallelConfig::default());
        // create primal module
        let mut primal_module = PrimalModuleParallel::new_config(&initializer, Arc::clone(&partition_info), PrimalModuleParallelConfig::default());
        // try to work on a simple syndrome
        code.set_syndrome(&syndrome_vertices);
        let mut interface = DualModuleInterface::new(&code.get_syndrome(), &mut dual_module);
        dual_module.fuse_all();
        interface.debug_print_actions = true;
        primal_module.load(&interface);  // load syndrome and connect to the dual module interface
        visualizer.as_mut().map(|v| v.snapshot_combined(format!("syndrome"), vec![&interface, &dual_module, &primal_module]).unwrap());
        // grow until end
        let mut group_max_update_length = dual_module.compute_maximum_update_length();
        while !group_max_update_length.is_empty() {
            println!("group_max_update_length: {:?}", group_max_update_length);
            if let Some(length) = group_max_update_length.get_none_zero_growth() {
                interface.grow(length, &mut dual_module);
                visualizer.as_mut().map(|v| v.snapshot_combined(format!("grow {length}"), vec![&interface, &dual_module, &primal_module]).unwrap());
            } else {
                let first_conflict = format!("{:?}", group_max_update_length.peek().unwrap());
                primal_module.resolve(group_max_update_length, &mut interface, &mut dual_module);
                visualizer.as_mut().map(|v| v.snapshot_combined(format!("resolve {first_conflict}"), vec![&interface, &dual_module, &primal_module]).unwrap());
            }
            group_max_update_length = dual_module.compute_maximum_update_length();
        }
        assert_eq!(interface.sum_dual_variables, final_dual * 2, "unexpected final dual variable sum");
        (interface, primal_module, dual_module)
    }
    
    pub fn primal_module_parallel_standard_syndrome<F>(code: impl ExampleCode, visualize_filename: String, syndrome_vertices: Vec<VertexIndex>
            , final_dual: Weight, partition_func: F, reordered_vertices: Option<Vec<VertexIndex>>)
            -> (DualModuleInterface, PrimalModuleParallel, DualModuleParallel<DualModuleSerial>) where F: Fn(&SolverInitializer, &mut PartitionConfig) {
        primal_module_parallel_basic_standard_syndrome_optional_viz(code, Some(visualize_filename), syndrome_vertices, final_dual, partition_func, reordered_vertices)
    }

    /// test a simple case
    #[test]
    fn primal_module_parallel_basic_1() {  // cargo test primal_module_parallel_basic_1 -- --nocapture
        let visualize_filename = format!("primal_module_parallel_basic_1.json");
        let syndrome_vertices = vec![39, 52, 63, 90, 100];
        let half_weight = 500;
        primal_module_parallel_standard_syndrome(CodeCapacityPlanarCode::new(11, 0.1, half_weight), visualize_filename, syndrome_vertices, 9 * half_weight, |initializer, _config| {
            println!("initializer: {initializer:?}");
        }, None);
    }


}
