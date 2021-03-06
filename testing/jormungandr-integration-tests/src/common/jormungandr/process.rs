use super::JormungandrError;
use crate::common::jcli::JCli;
use assert_fs::TempDir;
use chain_impl_mockchain::fee::LinearFee;
use chain_time::TimeEra;
use jormungandr_lib::{
    crypto::hash::Hash,
    interfaces::{Block0Configuration, TrustedPeer},
};
use jormungandr_testing_utils::testing::{
    node::{
        uri_from_socket_addr, Explorer, JormungandrLogger, JormungandrRest,
        JormungandrStateVerifier,
    },
    JormungandrParams, SyncNode, TestConfig,
};
use jormungandr_testing_utils::testing::{RemoteJormungandr, RemoteJormungandrBuilder};
use std::net::SocketAddr;
use std::path::Path;
use std::process::Child;
use std::str::FromStr;

pub struct JormungandrProcess {
    pub child: Child,
    pub logger: JormungandrLogger,
    temp_dir: Option<TempDir>,
    alias: String,
    p2p_public_address: poldercast::Address,
    rest_socket_addr: SocketAddr,
    genesis_block_hash: Hash,
    block0_configuration: Block0Configuration,
    fees: LinearFee,
}

impl JormungandrProcess {
    pub(crate) fn from_config<Conf: TestConfig>(
        child: Child,
        params: &JormungandrParams<Conf>,
        temp_dir: Option<TempDir>,
        alias: String,
    ) -> Self {
        let log_file_path = params.log_file_path();
        let node_config = params.node_config();
        JormungandrProcess {
            child,
            temp_dir,
            alias,
            logger: JormungandrLogger::new(log_file_path),
            p2p_public_address: node_config.p2p_public_address(),
            rest_socket_addr: node_config.rest_socket_addr(),
            genesis_block_hash: Hash::from_str(params.genesis_block_hash()).unwrap(),
            block0_configuration: params.block0_configuration().clone(),
            fees: params.fees(),
        }
    }

    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub fn rest(&self) -> JormungandrRest {
        JormungandrRest::new(self.rest_uri())
    }

    pub fn secure_rest<P: AsRef<Path>>(&self, cert: P) -> JormungandrRest {
        JormungandrRest::new_with_cert(self.rest_uri(), cert)
    }

    pub fn shutdown(&self) {
        let jcli: JCli = Default::default();
        jcli.rest().v0().shutdown(self.rest_uri());
    }

    pub fn address(&self) -> poldercast::Address {
        self.p2p_public_address.clone()
    }

    pub fn correct_state_verifier(&self) -> JormungandrStateVerifier {
        JormungandrStateVerifier::new(self.rest())
    }

    pub fn log_stats(&self) {
        println!("{:?}", self.rest().stats());
    }

    pub fn assert_no_errors_in_log_with_message(&self, message: &str) {
        self.logger.assert_no_errors(message);
    }

    pub fn assert_no_errors_in_log(&self) {
        self.logger.assert_no_errors("");
    }

    pub fn check_no_errors_in_log(&self) -> Result<(), JormungandrError> {
        let error_lines = self.logger.get_lines_with_error().collect::<Vec<String>>();

        if !error_lines.is_empty() {
            return Err(JormungandrError::ErrorInLogs {
                logs: self.logger.get_log_content(),
                log_location: self.logger.log_file_path.clone(),
                error_lines: format!("{:?}", error_lines),
            });
        }
        Ok(())
    }

    pub fn rest_uri(&self) -> String {
        uri_from_socket_addr(self.rest_socket_addr)
    }

    pub fn fees(&self) -> LinearFee {
        self.fees
    }

    pub fn genesis_block_hash(&self) -> Hash {
        self.genesis_block_hash
    }

    pub fn block0_configuration(&self) -> &Block0Configuration {
        &self.block0_configuration
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn explorer(&self) -> Explorer {
        Explorer::new(self.rest_socket_addr.to_string())
    }

    pub fn to_trusted_peer(&self) -> TrustedPeer {
        TrustedPeer {
            address: self.p2p_public_address.clone(),
            id: None,
        }
    }

    pub fn time_era(&self) -> TimeEra {
        let block_date = self.explorer().current_time();

        TimeEra::new(
            (block_date.slot() as u64).into(),
            chain_time::Epoch(block_date.epoch()),
            self.block0_configuration
                .blockchain_configuration
                .slots_per_epoch
                .into(),
        )
    }

    pub fn to_remote(&self) -> RemoteJormungandr {
        let mut builder = RemoteJormungandrBuilder::new(self.alias.clone());
        builder
            .with_rest(self.rest_socket_addr)
            .with_logger(self.logger.log_file_path.clone());
        builder.build()
    }

    pub fn steal_temp_dir(&mut self) -> Option<TempDir> {
        self.temp_dir.take()
    }

    pub fn stop(mut self) {
        match self.child.kill() {
            Err(e) => println!("Could not kill {}: {}", self.alias, e),
            Ok(_) => {
                println!("Successfully killed {}", self.alias);
            }
        }
    }
}

impl Drop for JormungandrProcess {
    fn drop(&mut self) {
        // There's no kill like overkill
        let _ = self.child.kill();

        // FIXME: These should be better done in a test harness
        self.child.wait().unwrap();
        self.logger.print_error_and_invalid_logs();
    }
}

impl SyncNode for JormungandrProcess {
    fn alias(&self) -> &str {
        self.alias()
    }

    fn last_block_height(&self) -> u32 {
        let docs = self.rest().stats().unwrap();
        docs.stats
            .expect("no stats object in response")
            .last_block_height
            .expect("last_block_height field is missing")
            .parse()
            .unwrap()
    }

    fn log_stats(&self) {
        println!("{:?}", self.rest().stats());
    }

    fn tip(&self) -> Hash {
        self.rest().tip().expect("cannot get tip from rest")
    }

    fn log_content(&self) -> String {
        self.logger.get_log_content()
    }

    fn get_lines_with_error_and_invalid(&self) -> Vec<String> {
        self.logger.get_lines_with_error_and_invalid().collect()
    }

    fn is_running(&self) -> bool {
        todo!()
    }
}
