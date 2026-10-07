use std::fs;

use lgk_autosar::daemon::commands::CommandDispatcher;
use lgk_autosar::ops;
use lgk_autosar::project::SessionConfig;
use serde_json::json;
use tempfile::tempdir;

fn asw_bundle_request(config: &SessionConfig) -> serde_json::Value {
    let developer = config.project_path.join("Config/Developer");
    fs::create_dir_all(&developer).unwrap();
    let dpa = config.dpa_file().unwrap();
    let dpa_text = fs::read_to_string(&dpa).unwrap();
    fs::write(dpa, dpa_text.replace("<Folders>", "<Folders><ApplicationComponentFolders><ApplicationComponentFolder>Config/Developer</ApplicationComponentFolder></ApplicationComponentFolders>")).unwrap();
    fs::write(developer.join("Public.arxml"), r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Public</SHORT-NAME><ELEMENTS><IMPLEMENTATION-DATA-TYPE><SHORT-NAME>Byte</SHORT-NAME><CATEGORY>VALUE</CATEGORY></IMPLEMENTATION-DATA-TYPE><SW-BASE-TYPE><SHORT-NAME>Base</SHORT-NAME></SW-BASE-TYPE></ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#).unwrap();
    json!({"func":"write_asw_bundle","file":developer.join("Authored.arxml"),"expected":null,
        "bundle":{"package":"Authored",
            "types":[{"kind":"alias","name":"Counter","type_ref":"/Public/Byte"},
                {"kind":"scalar","name":"Scalar","base_type_ref":"/Public/Base"},
                {"kind":"array","name":"Buffer","type_ref":"/Authored/Counter","length":4},
                {"kind":"record","name":"Pair","fields":[{"name":"Count","type_ref":"/Authored/Counter"},{"name":"Bytes","type_ref":"/Authored/Buffer"}]}],
            "interfaces":[{"kind":"sender_receiver","name":"Values","data_elements":[{"name":"Count","type_ref":"/Authored/Counter"}]},
                {"kind":"client_server","name":"Api","operations":[{"name":"Get","arguments":[{"name":"Result","type_ref":"/Authored/Counter","direction":"OUT"}]}]}],
            "components":[{"name":"Producer","ports":[{"name":"Out","direction":"provide","interface_ref":"/Authored/Values"}],
                "runnables":[{"name":"Tick","symbol":"Producer_Tick","period_seconds":0.01,"writes":[{"port":"Out","data_element":"Count"}]}]},
                {"name":"Consumer","ports":[{"name":"In","direction":"require","interface_ref":"/Authored/Values"}],
                "runnables":[{"name":"Tick","symbol":"Consumer_Tick","reads":[{"port":"In","data_element":"Count"}]}]}],
            "compositions":[{"name":"Root","instances":[{"name":"Tx","type_ref":"/Authored/Producer"},{"name":"Rx","type_ref":"/Authored/Consumer"}],
                "ports":[{"name":"Out","direction":"provide","interface_ref":"/Authored/Values"}],
                "connections":[{"name":"Link","provider":{"instance":"Tx","port":"Out"},"requester":{"instance":"Rx","port":"In"}}],
                "delegations":[{"name":"Expose","inner":{"instance":"Tx","port":"Out"},"outer_port":"Out"}]}]}})
}

#[test]
fn asw_bundle_preview_create_update_delete_and_saved_mapping_trace() {
    let (_root, config) = fixture();
    let mut request = asw_bundle_request(&config);
    let file = config.project_path.join("Config/Developer/Authored.arxml");
    let mut dispatcher = CommandDispatcher::new();
    request["preview"] = json!(true);
    let preview = dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    assert!(preview["objects"].as_u64().unwrap() > 20);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["operation"], "create");
    assert!(!file.exists());
    request["preview"] = json!(false);
    let created = dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    assert_eq!(created["davinci_validated"], false);
    assert_eq!(created["applied"], true);
    let original = fs::read_to_string(&file).unwrap();
    assert!(original.contains("xmlns=\"http://autosar.org/schema/r4.0\""));
    let mapping = dispatcher
        .dispatch_batch(
            &config,
            r#"{"func":"inspect_autosar_mapping","category":"ports","summary_only":true}"#,
        )
        .unwrap();
    assert_eq!(mapping["count"], 2);
    assert_eq!(mapping["unresolved_references_in_scope"], 0);
    let trace = dispatcher
        .dispatch_batch(
            &config,
            r#"{"func":"trace_autosar_model","start":"/Authored/Producer","direction":"outgoing"}"#,
        )
        .unwrap();
    assert!(trace["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["path"] == "/Authored/Values"));
    request["expected"] = json!(original);
    request["bundle"]["components"][0]["runnables"][0]["period_seconds"] = json!(0.02);
    dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    let updated = fs::read_to_string(&file).unwrap();
    assert!(updated.contains("<PERIOD>0.02</PERIOD>"));
    let mut deletion = json!({"func":"write_asw_bundle","file":file,"expected":updated,"delete":true,"preview":true});
    let preview = dispatcher
        .dispatch_batch(&config, &deletion.to_string())
        .unwrap();
    assert_eq!(preview["deleted"], false);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["operation"], "delete");
    assert!(file.exists());
    deletion["preview"] = json!(false);
    dispatcher
        .dispatch_batch(&config, &deletion.to_string())
        .unwrap();
    assert!(!file.exists());
    dispatcher.shutdown().unwrap();
}

#[test]
fn asw_bundle_rejects_stale_unknown_outside_and_mutating_batch() {
    let (root, config) = fixture();
    let request = asw_bundle_request(&config);
    let mut dispatcher = CommandDispatcher::new();
    dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    let file = config.project_path.join("Config/Developer/Authored.arxml");
    let original = fs::read(&file).unwrap();
    assert!(dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap_err()
        .to_string()
        .contains("absent file"));
    let mut stale = request.clone();
    stale["expected"] = json!("stale");
    assert!(dispatcher
        .dispatch_batch(&config, &stale.to_string())
        .unwrap_err()
        .to_string()
        .contains("does not match expected"));
    let mut bad = request.clone();
    bad.as_object_mut().unwrap().remove("expected");
    assert!(CommandDispatcher::validate_batch(&config, &bad.to_string())
        .unwrap_err()
        .to_string()
        .contains("expected is required"));
    bad = request.clone();
    bad["bundle"]["typo"] = json!(true);
    assert!(CommandDispatcher::validate_batch(&config, &bad.to_string()).is_err());
    bad = request.clone();
    bad["file"] = json!(root.path().join("Outside.arxml"));
    assert!(dispatcher
        .dispatch_batch(&config, &bad.to_string())
        .is_err());
    let batch = json!([{"func":"find_module","module":"Com"},request]);
    assert!(dispatcher
        .dispatch_batch(&config, &batch.to_string())
        .unwrap_err()
        .to_string()
        .contains("standalone"));
    let dpa = config.dpa_file().unwrap();
    fs::write(&dpa, fs::read_to_string(&dpa).unwrap().replace("</ApplicationComponentFolders>",
        "<ApplicationComponentFolder>MissingInput</ApplicationComponentFolder></ApplicationComponentFolders>")).unwrap();
    bad = request.clone();
    bad["expected"] = json!(String::from_utf8(original.clone()).unwrap());
    assert!(dispatcher
        .dispatch_batch(&config, &bad.to_string())
        .unwrap_err()
        .to_string()
        .contains("inaccessible"));
    assert_eq!(fs::read(file).unwrap(), original);
}

#[test]
fn asw_bundle_uses_registered_inputs_outside_config_and_extends_reference_editing() {
    let (_root, config) = fixture();
    let mut request = asw_bundle_request(&config);
    let input = config.project_path.join("AswInputs");
    fs::create_dir_all(&input).unwrap();
    let dpa = config.dpa_file().unwrap();
    fs::write(
        &dpa,
        fs::read_to_string(&dpa)
            .unwrap()
            .replace("Config/Developer", "AswInputs"),
    )
    .unwrap();
    let mut dispatcher = CommandDispatcher::new();
    assert!(dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap_err()
        .to_string()
        .contains("ApplicationComponentFolder"));
    let file = input.join("Authored.arxml");
    request["file"] = json!(file);
    dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    for scope in ["model", "developer", "all"] {
        let inspection = json!({"func":"inspect_autosar_model","scope":scope,"path_prefix":"/Authored","summary_only":true});
        assert!(
            dispatcher
                .dispatch_batch(&config, &inspection.to_string())
                .unwrap()["total"]
                .as_u64()
                .unwrap()
                > 20
        );
    }
    let retarget = json!({"func":"set_asw_reference","file":file,"object_path":"/Authored/Counter","kind":"IMPLEMENTATION-DATA-TYPE",
        "role":"IMPLEMENTATION-DATA-TYPE-REF","expected":"/Public/Byte","value":"/Authored/Scalar"});
    dispatcher
        .dispatch_batch(&config, &retarget.to_string())
        .unwrap();
    assert!(fs::read_to_string(file)
        .unwrap()
        .contains("/Authored/Scalar</IMPLEMENTATION-DATA-TYPE-REF>"));
}

#[test]
fn asw_bundle_rejects_unresolved_dest_duplicate_and_incompatible_connectors() {
    let (_root, config) = fixture();
    let request = asw_bundle_request(&config);
    let file = config.project_path.join("Config/Developer/Authored.arxml");
    let mut dispatcher = CommandDispatcher::new();
    for (pointer, value) in [
        ("/bundle/types/0/type_ref", json!("/Missing/Type")),
        ("/bundle/types/0/type_ref", json!("/Public/Base")),
        ("/bundle/types/1/name", json!("Counter")),
        (
            "/bundle/compositions/0/connections/0/provider/instance",
            json!("Rx"),
        ),
        (
            "/bundle/components/1/ports/0/interface_ref",
            json!("/Authored/Api"),
        ),
        ("/bundle/components/0/runnables/0/period_seconds", json!(0)),
        (
            "/bundle/components/0/runnables/0/writes/0/port",
            json!("Unknown"),
        ),
        ("/bundle/types/2/length", json!(0)),
        ("/bundle/types/0/type_ref", json!("/Authored/Buffer")),
    ] {
        let mut bad = request.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            dispatcher
                .dispatch_batch(&config, &bad.to_string())
                .is_err(),
            "{pointer}"
        );
        assert!(!file.exists());
    }
    let public = config.project_path.join("Config/Developer/Public.arxml");
    fs::write(&public, fs::read_to_string(&public).unwrap().replace("<CATEGORY>VALUE</CATEGORY>",
        "<CATEGORY>TYPE_REFERENCE</CATEGORY><IMPLEMENTATION-DATA-TYPE-REF DEST=\"IMPLEMENTATION-DATA-TYPE\">/Authored/Counter</IMPLEMENTATION-DATA-TYPE-REF>")).unwrap();
    assert!(dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap_err()
        .to_string()
        .contains("cyclic"));
    assert!(!file.exists());
}

#[test]
fn asw_bundle_removal_preserves_external_references_and_foreign_files() {
    let (_root, config) = fixture();
    let request = asw_bundle_request(&config);
    let file = config.project_path.join("Config/Developer/Authored.arxml");
    let mut dispatcher = CommandDispatcher::new();
    dispatcher
        .dispatch_batch(&config, &request.to_string())
        .unwrap();
    let original = fs::read_to_string(&file).unwrap();
    fs::write(config.project_path.join("Config/Developer/Client.arxml"),r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Client</SHORT-NAME><ELEMENTS><IMPLEMENTATION-DATA-TYPE><SHORT-NAME>UsesCounter</SHORT-NAME><IMPLEMENTATION-DATA-TYPE-REF DEST="IMPLEMENTATION-DATA-TYPE">/Authored/Counter</IMPLEMENTATION-DATA-TYPE-REF></IMPLEMENTATION-DATA-TYPE></ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#).unwrap();
    let delete = json!({"func":"write_asw_bundle","file":file,"expected":original,"delete":true});
    assert!(dispatcher
        .dispatch_batch(&config, &delete.to_string())
        .unwrap_err()
        .to_string()
        .contains("dangling"));
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
    let mut update = request.clone();
    update["expected"] = json!(original);
    update["bundle"]["types"][0]["type_ref"] = json!("/Authored/Scalar");
    assert!(dispatcher
        .dispatch_batch(&config, &update.to_string())
        .unwrap_err()
        .to_string()
        .contains("externally referenced"));
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
    let foreign = config.project_path.join("Config/Developer/Public.arxml");
    let delete = json!({"func":"write_asw_bundle","file":foreign,"expected":fs::read_to_string(&foreign).unwrap(),"delete":true});
    assert!(dispatcher
        .dispatch_batch(&config, &delete.to_string())
        .unwrap_err()
        .to_string()
        .contains("LGK-authored"));
}

fn fixture() -> (tempfile::TempDir, SessionConfig) {
    let root = tempdir().expect("tempdir");
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::create_dir_all(project.join("Config/ECUC")).expect("project dirs");
    fs::create_dir_all(root.path().join("Gen/GenData")).expect("generated dirs");
    fs::create_dir_all(tool.join("BSWMD/Com")).expect("tool dirs");
    fs::create_dir_all(tool.join("DaVinciConfigurator/Core")).expect("DaVinci dirs");
    fs::write(tool.join("DaVinciConfigurator/Core/DVCfgCmd.exe"), b"")
        .expect("DaVinci command placeholder");
    fs::write(
        project.join("Test.dpa"),
        r#"<?xml version="1.0"?>
<ProjectAssistant>
  <Folders><GenData>..\Gen\GenData</GenData></Folders>
  <EcucSplitter>
    <Splitter File=".\Config\ECUC\Test_Com_ecuc.arxml">
      <Module Name="Com"/>
    </Splitter>
  </EcucSplitter>
</ProjectAssistant>"#,
    )
    .expect("dpa");
    fs::write(
        project.join("Config/ECUC/Test_Com_ecuc.arxml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <ECUC-MODULE-CONFIGURATION-VALUES>
    <SHORT-NAME>Com</SHORT-NAME>
    <DEFINITION-REF DEST="ECUC-MODULE-DEF">/MICROSAR/Com</DEFINITION-REF>
    <CONTAINERS>
      <ECUC-CONTAINER-VALUE>
        <SHORT-NAME>ComConfig</SHORT-NAME>
        <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/MICROSAR/Com/ComConfig</DEFINITION-REF>
        <SUB-CONTAINERS>
          <ECUC-CONTAINER-VALUE>
            <SHORT-NAME>SignalA</SHORT-NAME>
            <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/MICROSAR/Com/ComConfig/ComSignal</DEFINITION-REF>
            <PARAMETER-VALUES>
              <ECUC-NUMERICAL-PARAM-VALUE>
                <DEFINITION-REF DEST="ECUC-INTEGER-PARAM-DEF">/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition</DEFINITION-REF>
                <VALUE>8</VALUE>
              </ECUC-NUMERICAL-PARAM-VALUE>
            </PARAMETER-VALUES>
          </ECUC-CONTAINER-VALUE>
        </SUB-CONTAINERS>
      </ECUC-CONTAINER-VALUE>
    </CONTAINERS>
  </ECUC-MODULE-CONFIGURATION-VALUES>
</AUTOSAR>
"#,
    )
    .expect("config");
    fs::write(
        tool.join("BSWMD/Com/Com_bswmd.arxml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <AR-PACKAGES>
    <AR-PACKAGE>
      <SHORT-NAME>MICROSAR</SHORT-NAME>
      <ELEMENTS>
        <ECUC-MODULE-DEF>
          <SHORT-NAME>Com</SHORT-NAME>
          <CONTAINERS>
            <ECUC-PARAM-CONF-CONTAINER-DEF>
              <SHORT-NAME>ComConfig</SHORT-NAME>
              <SUB-CONTAINERS>
                <ECUC-PARAM-CONF-CONTAINER-DEF>
                  <SHORT-NAME>ComSignal</SHORT-NAME>
                  <PARAMETERS>
                    <ECUC-INTEGER-PARAM-DEF>
                      <SHORT-NAME>ComBitPosition</SHORT-NAME>
                      <DESC><L-2 L="EN">Starting position in the I-PDU.</L-2></DESC>
                      <DEFAULT-VALUE>0</DEFAULT-VALUE>
                      <MIN>0</MIN>
                      <MAX>65535</MAX>
                    </ECUC-INTEGER-PARAM-DEF>
                  </PARAMETERS>
                </ECUC-PARAM-CONF-CONTAINER-DEF>
                <ECUC-CHOICE-CONTAINER-DEF>
                  <SHORT-NAME>ComGwDestination</SHORT-NAME>
                  <LOWER-MULTIPLICITY>0</LOWER-MULTIPLICITY>
                  <CHOICES>
                    <ECUC-PARAM-CONF-CONTAINER-DEF>
                      <SHORT-NAME>ComGwSignal</SHORT-NAME>
                      <PARAMETERS>
                        <ECUC-INTEGER-PARAM-DEF>
                          <SHORT-NAME>ComGwSignalBitPosition</SHORT-NAME>
                          <MAX>4095</MAX>
                        </ECUC-INTEGER-PARAM-DEF>
                      </PARAMETERS>
                    </ECUC-PARAM-CONF-CONTAINER-DEF>
                  </CHOICES>
                </ECUC-CHOICE-CONTAINER-DEF>
              </SUB-CONTAINERS>
            </ECUC-PARAM-CONF-CONTAINER-DEF>
          </CONTAINERS>
        </ECUC-MODULE-DEF>
      </ELEMENTS>
    </AR-PACKAGE>
  </AR-PACKAGES>
</AUTOSAR>"#,
    )
    .expect("template");
    fs::write(
        project.join("lgk-autosar.json"),
        format!(
            "{{\"project_path\":{},\"tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("config file");
    let config = SessionConfig::load(&project).expect("load session");
    (root, config)
}

fn write_autosar_model_fixture(config: &SessionConfig) {
    let system = config.project_path.join("Config/System");
    let developer = config.project_path.join("Config/Developer");
    fs::create_dir_all(&system).expect("system model dir");
    fs::create_dir_all(&developer).expect("developer model dir");
    fs::write(
        system.join("Communication.arxml"),
        r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Signals</SHORT-NAME><ELEMENTS>
        <SYSTEM-SIGNAL><SHORT-NAME>VehicleSpeed</SHORT-NAME></SYSTEM-SIGNAL>
        <I-SIGNAL><SHORT-NAME>ISignalVehicleSpeed</SHORT-NAME><SYSTEM-SIGNAL-REF DEST="SYSTEM-SIGNAL">/Signals/VehicleSpeed</SYSTEM-SIGNAL-REF></I-SIGNAL>
        <I-SIGNAL-I-PDU><SHORT-NAME>SpeedPdu</SHORT-NAME><I-SIGNAL-TO-I-PDU-MAPPINGS><I-SIGNAL-TO-I-PDU-MAPPING><SHORT-NAME>SpeedMap</SHORT-NAME><I-SIGNAL-REF DEST="I-SIGNAL">/Signals/ISignalVehicleSpeed</I-SIGNAL-REF></I-SIGNAL-TO-I-PDU-MAPPING></I-SIGNAL-TO-I-PDU-MAPPINGS></I-SIGNAL-I-PDU>
        <CAN-FRAME><SHORT-NAME>SpeedFrame</SHORT-NAME><PDU-TO-FRAME-MAPPINGS><PDU-TO-FRAME-MAPPING><SHORT-NAME>FrameMap</SHORT-NAME><PDU-REF DEST="I-SIGNAL-I-PDU">/Signals/SpeedPdu</PDU-REF></PDU-TO-FRAME-MAPPING></PDU-TO-FRAME-MAPPINGS></CAN-FRAME>
        </ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#,
    )
    .expect("system model");
    fs::write(
        developer.join("Software.arxml"),
        r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Application</SHORT-NAME><ELEMENTS>
        <APPLICATION-SW-COMPONENT-TYPE><SHORT-NAME>SpeedConsumer</SHORT-NAME><PORTS><R-PORT-PROTOTYPE><SHORT-NAME>SpeedPort</SHORT-NAME><REQUIRED-INTERFACE-TREF DEST="SENDER-RECEIVER-INTERFACE">/Application/SpeedInterface</REQUIRED-INTERFACE-TREF></R-PORT-PROTOTYPE></PORTS></APPLICATION-SW-COMPONENT-TYPE>
        <SENDER-RECEIVER-INTERFACE><SHORT-NAME>SpeedInterface</SHORT-NAME><DATA-ELEMENTS><VARIABLE-DATA-PROTOTYPE><SHORT-NAME>Speed</SHORT-NAME><TYPE-TREF DEST="IMPLEMENTATION-DATA-TYPE">/Application/SpeedType</TYPE-TREF></VARIABLE-DATA-PROTOTYPE></DATA-ELEMENTS></SENDER-RECEIVER-INTERFACE>
        <SENDER-RECEIVER-INTERFACE><SHORT-NAME>NewSpeedInterface</SHORT-NAME></SENDER-RECEIVER-INTERFACE>
        <IMPLEMENTATION-DATA-TYPE><SHORT-NAME>SpeedType</SHORT-NAME></IMPLEMENTATION-DATA-TYPE>
        <DATA-MAPPINGS><SHORT-NAME>SpeedDataMapping</SHORT-NAME><SYSTEM-SIGNAL-REF DEST="SYSTEM-SIGNAL">/Signals/VehicleSpeed</SYSTEM-SIGNAL-REF><DATA-ELEMENT-REF DEST="VARIABLE-DATA-PROTOTYPE">/Application/SpeedInterface/Speed</DATA-ELEMENT-REF></DATA-MAPPINGS>
        </ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#,
    )
    .expect("developer model");
}

#[test]
fn inspect_autosar_model_indexes_system_and_developer_objects() {
    let (_root, config) = fixture();
    write_autosar_model_fixture(&config);
    let result = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"inspect_autosar_model","kinds":["CAN-FRAME","APPLICATION-SW-COMPONENT-TYPE"],"limit":8}"#,
        )
        .expect("inspect model");
    assert_eq!(result["total"], 2);
    assert_eq!(result["files_scanned"], 2);
}

#[test]
fn summarize_large_autosar_model_without_returning_every_signal() {
    let (_root, config) = fixture();
    let system = config.project_path.join("Config/System");
    fs::create_dir_all(&system).expect("system directory");
    let mut model = String::from(
        "<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Signals</SHORT-NAME><ELEMENTS>",
    );
    for index in 0..10_000 {
        model.push_str(&format!(
            "<SYSTEM-SIGNAL><SHORT-NAME>Signal{index}</SHORT-NAME></SYSTEM-SIGNAL>"
        ));
    }
    model.push_str("</ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>");
    fs::write(system.join("Bulk.arxml"), model).expect("large model");

    let request = r#"{"func":"inspect_autosar_model","scope":"system","kinds":["SYSTEM-SIGNAL"],"summary_only":true}"#;
    CommandDispatcher::validate_batch(&config, request).expect("summary preflight");
    let result = CommandDispatcher::new()
        .dispatch_batch(&config, request)
        .expect("summary");
    assert_eq!(result["total"], 10_000);
    assert_eq!(result["kind_count"], 1);
    assert_eq!(result["by_kind"][0]["kind"], "SYSTEM-SIGNAL");
    assert_eq!(result["by_kind"][0]["count"], 10_000);
    assert!(result.get("objects").is_none());
    assert!(serde_json::to_vec(&result).expect("json").len() < 1024);

    let audit = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"audit_autosar_model","scope":"system","kinds":["SYSTEM-SIGNAL"]}"#,
        )
        .expect("large model audit");
    assert_eq!(audit["unresolved_in_scope"], 0);
    assert!(serde_json::to_vec(&audit).expect("audit json").len() < 1024);

    let error = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"inspect_autosar_model","summary_only":"yes"}"#,
        )
        .expect_err("invalid summary flag must fail");
    assert!(error.to_string().contains("summary_only must be a boolean"));
}

#[test]
fn audits_saved_autosar_references_with_bounded_examples() {
    let (_root, config) = fixture();
    write_autosar_model_fixture(&config);
    let file = config.project_path.join("Config/Developer/Software.arxml");
    let source = fs::read_to_string(&file).expect("developer model");
    let changed = source
        .replace(
            ">/Application/SpeedInterface</REQUIRED-INTERFACE-TREF>",
            ">/Application/MissingInterface</REQUIRED-INTERFACE-TREF>",
        )
        .replace(
            ">/Application/SpeedType</TYPE-TREF>",
            ">/Application/MissingType</TYPE-TREF>",
        );
    fs::write(&file, changed).expect("two broken references");

    let request =
        r#"{"func":"audit_autosar_model","scope":"model","path_prefix":"/Application","limit":1}"#;
    CommandDispatcher::validate_batch(&config, request).expect("audit preflight");
    let result = CommandDispatcher::new()
        .dispatch_batch(&config, request)
        .expect("audit model");
    assert_eq!(result["unresolved_in_scope"], 2);
    assert_eq!(result["examples"].as_array().expect("examples").len(), 1);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["by_role"].as_array().expect("roles").len(), 2);
    assert!(result["references_checked"].as_u64().expect("count") >= 4);
    assert!(serde_json::to_vec(&result).expect("json").len() < 1024);

    let long_target = format!("/Application/{}", "X".repeat(12_000));
    fs::write(
        &file,
        fs::read_to_string(&file)
            .expect("model")
            .replace("/Application/MissingInterface", &long_target),
    )
    .expect("long unresolved target");
    let bounded = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"audit_autosar_model","kinds":["R-PORT-PROTOTYPE"]}"#,
        )
        .expect("bounded audit");
    assert_eq!(bounded["unresolved_in_scope"], 1);
    assert_eq!(bounded["examples"][0]["fields_truncated"], true);
    assert!(
        bounded["examples"][0]["target"]
            .as_str()
            .expect("target")
            .chars()
            .count()
            <= 512
    );
    assert!(serde_json::to_vec(&bounded).expect("bounded json").len() < 2048);
}

#[test]
fn trace_autosar_model_crosses_references_and_containment() {
    let (_root, config) = fixture();
    write_autosar_model_fixture(&config);
    let result = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"trace_autosar_model","start":"/Signals/VehicleSpeed","direction":"both","depth":5}"#,
        )
        .expect("trace model");
    let paths = result["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter_map(|node| node["path"].as_str())
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/Signals/SpeedFrame"));
    assert!(paths.contains(&"/Application/SpeedDataMapping"));

    let asw = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"trace_autosar_model","start":"/Application/SpeedConsumer/SpeedPort","direction":"outgoing","depth":4}"#,
        )
        .expect("trace ASW type chain");
    let asw_paths = asw["nodes"]
        .as_array()
        .expect("ASW nodes")
        .iter()
        .filter_map(|node| node["path"].as_str())
        .collect::<Vec<_>>();
    assert!(asw_paths.contains(&"/Application/SpeedInterface"));
    assert!(asw_paths.contains(&"/Application/SpeedType"));
}

#[test]
fn semantic_asw_reference_preview_write_and_preconditions() {
    let (_root, config) = fixture();
    write_autosar_model_fixture(&config);
    let file = config.project_path.join("Config/Developer/Software.arxml");
    let request = json!({
        "func": "set_asw_reference",
        "file": file,
        "object_path": "/Application/SpeedConsumer/SpeedPort",
        "kind": "R-PORT-PROTOTYPE",
        "role": "REQUIRED-INTERFACE-TREF",
        "expected": "/Application/SpeedInterface",
        "value": "/Application/NewSpeedInterface",
        "preview": true
    });
    let raw = request.to_string();
    CommandDispatcher::validate_batch(&config, &raw).expect("ASW preflight");
    let original = fs::read(&file).expect("original");
    let preview = CommandDispatcher::new()
        .dispatch_batch(&config, &raw)
        .expect("preview");
    assert_eq!(preview["preview"], true);
    assert_eq!(preview["changed"], true);
    assert_eq!(fs::read(&file).expect("unchanged preview"), original);

    let mut write = request.clone();
    write["preview"] = json!(false);
    let written = CommandDispatcher::new()
        .dispatch_batch(&config, &write.to_string())
        .expect("write ASW reference");
    assert_eq!(written["changed"], true);
    let changed = fs::read_to_string(&file).expect("changed ASW");
    assert!(changed.contains(">/Application/NewSpeedInterface</REQUIRED-INTERFACE-TREF>"));
    assert!(!changed.contains(">/Application/SpeedInterface</REQUIRED-INTERFACE-TREF>"));

    let stale = CommandDispatcher::new()
        .dispatch_batch(&config, &write.to_string())
        .expect_err("stale expected value must fail");
    assert!(stale.to_string().contains("does not match expected"));
    assert_eq!(fs::read_to_string(&file).expect("still changed"), changed);

    let mut missing = request;
    missing["expected"] = json!("/Application/NewSpeedInterface");
    missing["value"] = json!("/Application/AbsentInterface");
    let error = CommandDispatcher::new()
        .dispatch_batch(&config, &missing.to_string())
        .expect_err("missing target must fail");
    assert!(error.to_string().contains("target was not found"));

    missing["value"] = json!("/Application/SpeedType");
    let wrong_kind = CommandDispatcher::new()
        .dispatch_batch(&config, &missing.to_string())
        .expect_err("existing target with wrong DEST kind must fail");
    assert!(wrong_kind.to_string().contains("required DEST kind"));
}

#[test]
fn semantic_asw_reference_ignores_comment_and_rejects_batch_or_outside_file() {
    let (_root, config) = fixture();
    write_autosar_model_fixture(&config);
    let file = config.project_path.join("Config/Developer/Software.arxml");
    let source = fs::read_to_string(&file).expect("source");
    fs::write(
        &file,
        format!(
            "<!-- <REQUIRED-INTERFACE-TREF>/Application/SpeedInterface</REQUIRED-INTERFACE-TREF> -->\n{source}"
        ),
    )
    .expect("commented decoy");
    let request = json!({
        "func": "set_asw_reference",
        "file": file,
        "object_path": "/Application/SpeedConsumer/SpeedPort",
        "kind": "R-PORT-PROTOTYPE",
        "role": "REQUIRED-INTERFACE-TREF",
        "expected": "/Application/SpeedInterface",
        "value": "/Application/NewSpeedInterface"
    });
    let batch = json!([request.clone(), {"func": "find_module", "module": "Com"}]);
    assert!(
        CommandDispatcher::validate_batch(&config, &batch.to_string())
            .expect_err("write in a batch")
            .to_string()
            .contains("standalone")
    );
    CommandDispatcher::new()
        .dispatch_batch(&config, &request.to_string())
        .expect("target real reference, not comment");
    let updated = fs::read_to_string(&file).expect("updated");
    assert!(updated.starts_with(
        "<!-- <REQUIRED-INTERFACE-TREF>/Application/SpeedInterface</REQUIRED-INTERFACE-TREF> -->"
    ));

    let outside = config
        .project_path
        .parent()
        .expect("parent")
        .join("Other.arxml");
    fs::write(&outside, "<AUTOSAR/>").expect("outside file");
    let mut rejected = request;
    rejected["file"] = json!(outside);
    assert!(CommandDispatcher::new()
        .dispatch_batch(&config, &rejected.to_string())
        .expect_err("outside file")
        .to_string()
        .contains("outside project_path"));
}

#[test]
fn accepts_utf8_bom_in_project_configuration() {
    let (root, config) = fixture();
    let config_path = config.project_path.join("lgk-autosar.json");
    let raw = fs::read_to_string(&config_path).expect("read config");
    fs::write(&config_path, format!("\u{feff}{raw}")).expect("write BOM config");
    let reloaded = SessionConfig::load(&config.project_path).expect("load BOM config");
    assert_eq!(reloaded.project_path, config.project_path);
    drop(root);
}

#[test]
fn finds_module_and_definition() {
    let (_root, config) = fixture();
    let module =
        ops::find_module::execute(&config, &json!({"module": "Com"})).expect("find module");
    assert_eq!(module["module"], "Com");

    let definition = ops::get_param_definition::execute(
        &config,
        &json!({"module": "Com", "params": "ComBitPosition"}),
    )
    .expect("get definition");
    assert_eq!(
        definition["definitions"][0]["definition_ref"],
        "/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition"
    );
    assert_eq!(
        definition["definitions"][0]["value_tag"],
        "ECUC-NUMERICAL-PARAM-VALUE"
    );

    let template =
        ops::find_module_template::execute(&config, &json!({"module": "Com", "details": true}))
            .expect("find module template");
    let definitions = template["definitions"].as_array().expect("definitions");
    let container = definitions
        .iter()
        .find(|item| item["name"] == "ComSignal")
        .expect("ComSignal definition");
    assert_eq!(container["range"], json!({}));
    assert!(container["ref_target"].is_null());
    let parameter = definitions
        .iter()
        .find(|item| item["name"] == "ComBitPosition")
        .expect("ComBitPosition definition");
    assert_eq!(parameter["range"]["max"], "65535");
    let choice = definitions
        .iter()
        .find(|item| item["name"] == "ComGwDestination")
        .expect("choice container definition");
    assert_eq!(choice["group"], "containers");
    assert_eq!(choice["value_tag"], "ECUC-CONTAINER-VALUE");
    assert_eq!(choice["range"], json!({"lower_multiplicity": "0"}));
    let choice_child = definitions
        .iter()
        .find(|item| item["name"] == "ComGwSignal")
        .expect("choice child definition");
    assert_eq!(
        choice_child["definition_ref"],
        "/MICROSAR/Com/ComConfig/ComGwDestination/ComGwSignal"
    );

    let compact = ops::find_module_template::execute(&config, &json!({"module": "Com"}))
        .expect("compact module template");
    assert!(compact.get("definitions").is_none());
    assert_eq!(compact["containers"][0]["name"], "ComConfig");
    assert_eq!(
        compact["containers"][0]["subcontainers"][0]["name"],
        "ComGwDestination"
    );
    assert_eq!(
        compact["containers"][0]["subcontainers"][1]["name"],
        "ComSignal"
    );
}

#[test]
fn template_cache_invalidates_when_the_definition_file_changes() {
    let (_root, config) = fixture();
    ops::get_param_definition::execute(
        &config,
        &json!({"module": "Com", "params": "ComBitPosition"}),
    )
    .expect("prime template cache");

    let template_path = config.tool_path.join("BSWMD/Com/Com_bswmd.arxml");
    let raw = fs::read_to_string(&template_path).expect("read template");
    let updated = raw.replacen(
        "</PARAMETERS>",
        "<ECUC-INTEGER-PARAM-DEF><SHORT-NAME>ComAddedAfterCache</SHORT-NAME><MAX>9</MAX></ECUC-INTEGER-PARAM-DEF></PARAMETERS>",
        1,
    );
    fs::write(&template_path, updated).expect("update template");

    let definition = ops::get_param_definition::execute(
        &config,
        &json!({"module": "Com", "params": "ComAddedAfterCache"}),
    )
    .expect("reload changed template");
    assert_eq!(definition["definitions"][0]["range"]["max"], "9");
}

#[test]
fn locates_container_by_definition_and_name() {
    let (_root, config) = fixture();
    let result = ops::locate_container::execute(
        &config,
        &json!({
            "module": "Com",
            "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal",
            "short_name_regex": "^SignalA$"
        }),
    )
    .expect("locate");
    assert_eq!(result["count"], 1);
    assert_eq!(result["containers"][0]["short_name"], "SignalA");
    assert!(
        result["containers"][0]["start_line"].as_u64().unwrap()
            < result["containers"][0]["end_line"].as_u64().unwrap()
    );
}

#[test]
fn locates_container_when_xml_tags_are_wrapped_across_lines() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let raw = fs::read_to_string(&file).expect("read config");
    let wrapped = raw
        .replace(
            "<SHORT-NAME>SignalA</SHORT-NAME>",
            "<SHORT-NAME>\n              SignalA\n            </SHORT-NAME>",
        )
        .replace(
            "<DEFINITION-REF DEST=\"ECUC-PARAM-CONF-CONTAINER-DEF\">/MICROSAR/Com/ComConfig/ComSignal</DEFINITION-REF>",
            "<DEFINITION-REF\n              DEST=\"ECUC-PARAM-CONF-CONTAINER-DEF\">\n              /MICROSAR/Com/ComConfig/ComSignal\n            </DEFINITION-REF>",
        );
    fs::write(&file, wrapped).expect("write wrapped config");

    let result = ops::locate_container::execute(
        &config,
        &json!({
            "module": "Com",
            "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal",
            "short_name_regex": "^SignalA$"
        }),
    )
    .expect("locate wrapped XML");
    assert_eq!(result["count"], 1);
    assert_eq!(result["containers"][0]["short_name"], "SignalA");
}

#[test]
fn inspects_configured_container_values_without_starting_davinci() {
    let (_root, config) = fixture();
    let result = ops::inspect_ecuc_containers::execute(
        &config,
        &json!({
            "module": "Com",
            "container": "ComSignal",
            "short_name_regex": "^SignalA$",
            "params": ["ComBitPosition"]
        }),
    )
    .expect("inspect");

    assert_eq!(result.as_array().expect("array").len(), 1);
    assert_eq!(result[0]["short_name"], "SignalA");
    assert_eq!(result[0]["container_path"], "/ComConfig/SignalA");
    assert_eq!(result[0]["values"]["ComBitPosition"], "8");
}

#[test]
fn inspects_by_full_definition_ref_and_comma_separated_params() {
    let (_root, config) = fixture();
    let result = ops::inspect_ecuc_containers::execute(
        &config,
        &json!({
            "module": "Com",
            "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal",
            "params": "ComBitPosition, MissingParameter"
        }),
    )
    .expect("inspect");

    assert_eq!(
        result[0]["definition_ref"],
        "/MICROSAR/Com/ComConfig/ComSignal"
    );
    assert_eq!(result[0]["values"]["ComBitPosition"], "8");
    assert!(result[0]["values"].get("MissingParameter").is_none());
}

#[test]
fn aggregates_multiple_inspection_requests_into_one_result_array() {
    let (_root, config) = fixture();
    let raw = serde_json::to_string(&json!([
        {
            "func": "inspect_ecuc_containers",
            "module": "Com",
            "container": "ComSignal"
        },
        {
            "func": "inspect_ecuc_containers",
            "module": "Com",
            "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal"
        }
    ]))
    .expect("request JSON");
    let result = CommandDispatcher::new()
        .dispatch_batch(&config, &raw)
        .expect("batch inspect");

    assert_eq!(result.as_array().expect("flat array").len(), 2);
    assert_eq!(result[0]["short_name"], "SignalA");
    assert_eq!(result[1]["short_name"], "SignalA");
}

#[test]
fn inspection_requires_pages_for_large_results_and_preserves_continuation() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let rows = (0..65).map(|n| format!("<ECUC-CONTAINER-VALUE><SHORT-NAME>Signal{n:03}</SHORT-NAME><DEFINITION-REF>/Example/ComSignal</DEFINITION-REF></ECUC-CONTAINER-VALUE>"))
        .collect::<String>();
    let xml = format!("<AUTOSAR><CONTAINERS>{rows}</CONTAINERS></AUTOSAR>");
    fs::write(&file, &xml).unwrap();
    let mut request = json!({"module":"Com","container":"ComSignal"});
    let error = ops::inspect_ecuc_containers::execute(&config, &request)
        .unwrap_err()
        .to_string();
    assert!(error.contains("INSPECTION_PAGE_REQUIRED"));
    request["paged"] = json!(true);
    request["limit"] = json!(8);
    let page = ops::inspect_ecuc_containers::execute(&config, &request).unwrap();
    assert_eq!(page["count"], 65);
    assert_eq!(page["containers"].as_array().unwrap().len(), 8);
    assert_eq!(page["next_offset"], 8);
    request["offset"] = page["next_offset"].clone();
    let next = ops::inspect_ecuc_containers::execute(&config, &request).unwrap();
    assert_eq!(next["containers"][0]["short_name"], "Signal008");
    request["offset"] = json!(64);
    let last = ops::inspect_ecuc_containers::execute(&config, &request).unwrap();
    assert_eq!(last["containers"].as_array().unwrap().len(), 1);
    assert_eq!(last["truncated"], false);
    assert!(last["next_offset"].is_null());
    assert_eq!(fs::read_to_string(file).unwrap(), xml);
    for invalid in [
        json!({"limit":0}),
        json!({"limit":257}),
        json!({"offset":-1}),
        json!({"paged":"yes"}),
        json!({"offset":1}),
    ] {
        let mut bad = json!({"module":"Com","container":"ComSignal"});
        for (key, value) in invalid.as_object().unwrap() {
            bad[key] = value.clone();
        }
        assert!(ops::inspect_ecuc_containers::execute(&config, &bad).is_err());
    }
}

#[test]
fn inspection_rejects_oversized_values_without_silent_truncation() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let xml = fs::read_to_string(&file).unwrap().replace(
        "<VALUE>8</VALUE>",
        &format!("<VALUE>{}</VALUE>", "a".repeat(70_000)),
    );
    fs::write(&file, &xml).unwrap();
    let error = ops::inspect_ecuc_containers::execute(
        &config,
        &json!({"module":"Com","container":"ComSignal","paged":true}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("64 KiB"));
    assert_eq!(fs::read_to_string(file).unwrap(), xml);
}

fn mapping_fixture(config: &SessionConfig) {
    let model = config.project_path.join("Config/Developer/Mapping.arxml");
    fs::create_dir_all(model.parent().unwrap()).unwrap();
    let connectors = ["I1", "I2"].into_iter().map(|instance| format!(r#"<ASSEMBLY-SW-CONNECTOR><SHORT-NAME>Link{instance}</SHORT-NAME>
      <PROVIDER-IREF><CONTEXT-COMPONENT-REF>/Example/Top/{instance}</CONTEXT-COMPONENT-REF><TARGET-P-PORT-REF DEST="P-PORT-PROTOTYPE">/Example/App/P</TARGET-P-PORT-REF></PROVIDER-IREF>
      <REQUESTER-IREF><CONTEXT-COMPONENT-REF>/Example/Top/Consumer</CONTEXT-COMPONENT-REF><TARGET-R-PORT-REF DEST="R-PORT-PROTOTYPE">/Example/App/R</TARGET-R-PORT-REF></REQUESTER-IREF>
      </ASSEMBLY-SW-CONNECTOR>"#)).collect::<String>();
    let data_rows = (0..2).map(|_| r#"<SENDER-RECEIVER-TO-SIGNAL-MAPPING><DATA-ELEMENT-IREF><CONTEXT-COMPONENT-REF>/Example/Top/I1</CONTEXT-COMPONENT-REF><CONTEXT-PORT-REF>/Example/App/P</CONTEXT-PORT-REF><TARGET-DATA-PROTOTYPE-REF>/Example/Data</TARGET-DATA-PROTOTYPE-REF></DATA-ELEMENT-IREF><SYSTEM-SIGNAL-REF>/Example/Signal</SYSTEM-SIGNAL-REF></SENDER-RECEIVER-TO-SIGNAL-MAPPING>"#).collect::<String>();
    fs::write(model, format!(r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Example</SHORT-NAME><ELEMENTS>
    <APPLICATION-SW-COMPONENT-TYPE><SHORT-NAME>App</SHORT-NAME><PORTS><P-PORT-PROTOTYPE><SHORT-NAME>P</SHORT-NAME></P-PORT-PROTOTYPE><R-PORT-PROTOTYPE><SHORT-NAME>R</SHORT-NAME></R-PORT-PROTOTYPE></PORTS><INTERNAL-BEHAVIORS><SWC-INTERNAL-BEHAVIOR><SHORT-NAME>IB</SHORT-NAME><EVENTS><TIMING-EVENT><SHORT-NAME>Tick</SHORT-NAME></TIMING-EVENT></EVENTS></SWC-INTERNAL-BEHAVIOR></INTERNAL-BEHAVIORS></APPLICATION-SW-COMPONENT-TYPE>
    <COMPOSITION-SW-COMPONENT-TYPE><SHORT-NAME>Top</SHORT-NAME><COMPONENTS><SW-COMPONENT-PROTOTYPE><SHORT-NAME>I1</SHORT-NAME></SW-COMPONENT-PROTOTYPE><SW-COMPONENT-PROTOTYPE><SHORT-NAME>I2</SHORT-NAME></SW-COMPONENT-PROTOTYPE><SW-COMPONENT-PROTOTYPE><SHORT-NAME>Consumer</SHORT-NAME></SW-COMPONENT-PROTOTYPE></COMPONENTS><CONNECTORS>{connectors}</CONNECTORS></COMPOSITION-SW-COMPONENT-TYPE>
    <VARIABLE-DATA-PROTOTYPE><SHORT-NAME>Data</SHORT-NAME></VARIABLE-DATA-PROTOTYPE><SYSTEM-SIGNAL><SHORT-NAME>Signal</SHORT-NAME></SYSTEM-SIGNAL><SYSTEM-MAPPING><SHORT-NAME>Mapping</SHORT-NAME><MAPPINGS>{data_rows}</MAPPINGS></SYSTEM-MAPPING>
    </ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#)).unwrap();
    let dpa = config.project_path.join("Test.dpa");
    let text = fs::read_to_string(&dpa).unwrap().replace("<EcucSplitter>", r#"<EcucSplitter><Splitter File="Config/ECUC/Mapping_ecuc.arxml"><Module Name="Rte"/><Module Name="Os"/></Splitter>"#);
    fs::write(dpa, text).unwrap();
    fn reference(role: &str, target: &str, dest: &str) -> String {
        format!(
            r#"<ECUC-REFERENCE-VALUE><DEFINITION-REF>/Example/Rte/{role}</DEFINITION-REF><VALUE-REF DEST="{dest}">{target}</VALUE-REF></ECUC-REFERENCE-VALUE>"#
        )
    }
    let swc = reference("RteEventRef", "/Example/App/IB/Tick", "TIMING-EVENT")
        + &reference(
            "RteMappedToTaskRef",
            "/ActiveEcuC/Os/Task",
            "ECUC-CONTAINER-VALUE",
        );
    let bsw = reference("RteBswEventRef", "/OutsideSip/BswEvent", "BSW-TIMING-EVENT");
    let instance = reference(
        "RteSoftwareComponentInstanceRef",
        "/Example/Top/I1",
        "SW-COMPONENT-PROTOTYPE",
    );
    fs::write(config.project_path.join("Config/ECUC/Mapping_ecuc.arxml"),format!(r#"<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>ActiveEcuC</SHORT-NAME><ELEMENTS>
    <ECUC-MODULE-CONFIGURATION-VALUES><SHORT-NAME>Rte</SHORT-NAME><CONTAINERS><ECUC-CONTAINER-VALUE><SHORT-NAME>Instance</SHORT-NAME><DEFINITION-REF>/Example/Rte/RteSwComponentInstance</DEFINITION-REF><REFERENCE-VALUES>{instance}</REFERENCE-VALUES><SUB-CONTAINERS><ECUC-CONTAINER-VALUE><SHORT-NAME>SwcMap</SHORT-NAME><DEFINITION-REF>/Example/Rte/RteEventToTaskMapping</DEFINITION-REF><REFERENCE-VALUES>{swc}</REFERENCE-VALUES><PARAMETER-VALUES><ECUC-NUMERICAL-PARAM-VALUE><DEFINITION-REF>/Example/Rte/RtePositionInTask</DEFINITION-REF><VALUE>3</VALUE></ECUC-NUMERICAL-PARAM-VALUE></PARAMETER-VALUES></ECUC-CONTAINER-VALUE></SUB-CONTAINERS></ECUC-CONTAINER-VALUE>
    <ECUC-CONTAINER-VALUE><SHORT-NAME>BswMap</SHORT-NAME><DEFINITION-REF>/Example/Rte/RteBswEventToTaskMapping</DEFINITION-REF><REFERENCE-VALUES>{bsw}</REFERENCE-VALUES></ECUC-CONTAINER-VALUE></CONTAINERS></ECUC-MODULE-CONFIGURATION-VALUES>
    <ECUC-MODULE-CONFIGURATION-VALUES><SHORT-NAME>Os</SHORT-NAME><CONTAINERS><ECUC-CONTAINER-VALUE><SHORT-NAME>Task</SHORT-NAME><DEFINITION-REF>/Example/Os/OsTask</DEFINITION-REF></ECUC-CONTAINER-VALUE></CONTAINERS></ECUC-MODULE-CONFIGURATION-VALUES>
    </ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>"#)).unwrap();
}

#[test]
fn mapping_keeps_anonymous_rows_and_port_instance_reference_locations() {
    let (_root, config) = fixture();
    mapping_fixture(&config);
    let result = CommandDispatcher::new()
        .dispatch_batch(
            &config,
            r#"{"func":"inspect_autosar_mapping","category":"ports"}"#,
        )
        .unwrap();
    assert_eq!(result["count"], 2);
    assert_eq!(result["unresolved_references_in_scope"], 0);
    let rows = result["rows"].as_array().unwrap();
    assert_ne!(rows[0]["owner_path"], rows[1]["owner_path"]);
    let refs = rows[0]["references"].as_array().unwrap();
    assert!(refs.iter().any(|r| r["target"] == "/Example/Top/I1"
        && r["xml_location"]
            .as_str()
            .unwrap()
            .contains("PROVIDER-IREF")));
    assert!(refs.iter().any(|r| r["target"] == "/Example/Top/Consumer"
        && r["xml_location"]
            .as_str()
            .unwrap()
            .contains("REQUESTER-IREF")));
    let result = ops::autosar_model::inspect_mapping(&config, &json!({"category":"data"})).unwrap();
    assert_eq!(result["count"], 2);
    assert_eq!(
        result["rows"][0]["owner_path"],
        result["rows"][1]["owner_path"]
    );
    assert_ne!(
        result["rows"][0]["xml_location"],
        result["rows"][1]["xml_location"]
    );
}

#[test]
fn mapping_reports_swc_bsw_and_missing_tasks_without_validation_claims() {
    let (_root, config) = fixture();
    mapping_fixture(&config);
    let file = config.project_path.join("Config/ECUC/Mapping_ecuc.arxml");
    let before = fs::read(&file).unwrap();
    let result =
        ops::autosar_model::inspect_mapping(&config, &json!({"category":"tasks"})).unwrap();
    assert_eq!(result["count"], 2);
    assert_eq!(result["ecuc"]["files_scanned"], 1);
    assert_eq!(result["rows_without_task_reference"], 1);
    let rows = result["rows"].as_array().unwrap();
    let swc = rows
        .iter()
        .find(|r| r["kind"] == "RteEventToTaskMapping")
        .unwrap();
    assert_eq!(swc["fields"]["RtePositionInTask"], "3");
    assert_eq!(swc["task_assignment"], "task_reference_present");
    assert_eq!(swc["instance_context"][0]["target"], "/Example/Top/I1");
    assert!(swc["references"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["resolution"] == "present_in_scope"));
    let bsw = rows
        .iter()
        .find(|r| r["kind"] == "RteBswEventToTaskMapping")
        .unwrap();
    assert_eq!(bsw["task_assignment"], "no_task_reference_requires_review");
    assert_eq!(result["unresolved_references_in_scope"], 1);
    let issues = ops::autosar_model::inspect_mapping(
        &config,
        &json!({"category":"tasks","issues_only":true}),
    )
    .unwrap();
    assert_eq!(issues["count"], 1);
    assert_eq!(fs::read(&file).unwrap(), before);
    fs::write(
        &file,
        String::from_utf8(before)
            .unwrap()
            .replace("DEST=\"TIMING-EVENT\"", "DEST=\"INIT-EVENT\""),
    )
    .unwrap();
    let mismatch =
        ops::autosar_model::inspect_mapping(&config, &json!({"category":"tasks"})).unwrap();
    assert_eq!(mismatch["dest_mismatches_in_scope"], 1);
}

#[test]
fn mapping_bounds_summary_pages_and_rejects_invalid_input() {
    let (_root, config) = fixture();
    let path = config.project_path.join("Config/System/Large.arxml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let row="<SENDER-RECEIVER-TO-SIGNAL-MAPPING><TARGET-DATA-PROTOTYPE-REF>/External/Data</TARGET-DATA-PROTOTYPE-REF><SYSTEM-SIGNAL-REF>/External/Signal</SYSTEM-SIGNAL-REF></SENDER-RECEIVER-TO-SIGNAL-MAPPING>";
    fs::write(&path, format!("<AUTOSAR>{}</AUTOSAR>", row.repeat(1000))).unwrap();
    let summary = ops::autosar_model::inspect_mapping(
        &config,
        &json!({"category":"data","summary_only":true}),
    )
    .unwrap();
    assert_eq!(summary["count"], 1000);
    assert!(summary["rows"].as_array().unwrap().is_empty());
    assert!(serde_json::to_vec(&summary).unwrap().len() < 2048);
    let last = ops::autosar_model::inspect_mapping(
        &config,
        &json!({"category":"data","limit":8,"offset":999}),
    )
    .unwrap();
    assert_eq!(last["rows"].as_array().unwrap().len(), 1);
    assert_eq!(last["truncated"], false);
    for bad in [
        json!({"category":"unknown"}),
        json!({"limit":33}),
        json!({"offset":-1}),
        json!({"summary_only":"yes"}),
        json!({"issues_only":1}),
        json!({"path_prefix":false}),
    ] {
        assert!(ops::autosar_model::inspect_mapping(&config, &bad).is_err());
    }
    let absent =
        ops::autosar_model::inspect_mapping(&config, &json!({"category":"tasks"})).unwrap();
    assert_eq!(absent["ecuc"]["rte_available"], false);
    assert_eq!(absent["ecuc"]["os_available"], false);
}

#[test]
fn mapping_and_trace_follow_leaf_refs_inside_delegation_instance_wrappers() {
    let (_root, config) = fixture();
    mapping_fixture(&config);
    let file = config.project_path.join("Config/Developer/Mapping.arxml");
    let xml=fs::read_to_string(&file).unwrap().replace("</CONNECTORS>", r#"<DELEGATION-SW-CONNECTOR><SHORT-NAME>Delegation</SHORT-NAME><INNER-PORT-IREF><R-PORT-IN-COMPOSITION-INSTANCE-REF><CONTEXT-COMPONENT-REF>/Example/Top/I1</CONTEXT-COMPONENT-REF><TARGET-R-PORT-REF DEST="R-PORT-PROTOTYPE">/Example/App/R</TARGET-R-PORT-REF></R-PORT-IN-COMPOSITION-INSTANCE-REF></INNER-PORT-IREF><OUTER-PORT-REF DEST="R-PORT-PROTOTYPE">/Example/App/R</OUTER-PORT-REF></DELEGATION-SW-CONNECTOR></CONNECTORS>"#);
    fs::write(file, xml).unwrap();
    let result = ops::autosar_model::inspect_mapping(
        &config,
        &json!({"category":"ports","path_prefix":"/Example/Top/Delegation"}),
    )
    .unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["rows_with_missing_roles"], 0);
    assert!(result["rows"][0]["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["role"] == "TARGET-R-PORT-REF"));
    let trace = ops::autosar_model::trace(
        &config,
        &json!({"start":"/Example/Top/Delegation","direction":"outgoing"}),
    )
    .unwrap();
    assert!(trace["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["role"] == "TARGET-R-PORT-REF"));
}

#[test]
fn verifies_synchronized_delivery_and_required_generated_values() {
    let (root, config) = fixture();
    let generated = root.path().join("Generated/Can_Lcfg.c");
    let compiled = root.path().join("Proj_Code/Can_Lcfg.c");
    fs::create_dir_all(generated.parent().expect("generated parent")).expect("generated dir");
    fs::create_dir_all(compiled.parent().expect("compiled parent")).expect("compiled dir");
    let content = "Can_InitPortSel[0] = 1u;\nCanIsr_0();\n0xF0018200u\n";
    fs::write(&generated, content).expect("generated file");
    fs::write(&compiled, content).expect("compiled file");

    let result = ops::verify_delivery::execute(
        &config,
        &json!({
            "root": root.path(),
            "checks": [{
                "path": "Proj_Code/Can_Lcfg.c",
                "same_as": "Generated/Can_Lcfg.c",
                "must_contain": ["Can_InitPortSel[0] = 1u", "CanIsr_0", "0xF0018200u"],
                "must_not_contain": ["CanIsr_1"]
            }]
        }),
    )
    .expect("verified delivery");

    assert_eq!(result["passed"], true);
    assert_eq!(result["checks"][0]["synchronized"], true);
    assert_eq!(result["checks"][0]["missing_required"], json!([]));
    assert_eq!(result["checks"][0]["forbidden_found"], json!([]));
}

#[test]
fn reports_and_enforces_unsynchronized_or_invalid_delivery() {
    let (root, config) = fixture();
    let generated = root.path().join("Generated/Can_Lcfg.c");
    let compiled = root.path().join("Proj_Code/Can_Lcfg.c");
    fs::create_dir_all(generated.parent().expect("generated parent")).expect("generated dir");
    fs::create_dir_all(compiled.parent().expect("compiled parent")).expect("compiled dir");
    fs::write(&generated, "Can_InitPortSel[0] = 1u;\nCanIsr_0();\n").expect("generated file");
    fs::write(&compiled, "Can_InitPortSel[0] = 0u;\nCanIsr_1();\n").expect("compiled file");
    let request = json!({
        "root": root.path(),
        "checks": [{
            "path": "Proj_Code/Can_Lcfg.c",
            "same_as": "Generated/Can_Lcfg.c",
            "must_contain": ["Can_InitPortSel[0] = 1u", "CanIsr_0"],
            "must_not_contain": ["CanIsr_1"]
        }]
    });

    let error = ops::verify_delivery::execute(&config, &request)
        .expect_err("default enforcement must reject stale compiled output");
    assert!(error.to_string().contains("delivery verification failed"));
    let failure: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
    assert_eq!(failure["code"], "DELIVERY_VERIFICATION_FAILED");
    assert_eq!(failure["success"], false);
    assert_eq!(failure["details"]["failed_checks"][0]["check_index"], 0);
    assert_eq!(
        failure["details"]["failed_checks"][0]["forbidden_found"],
        json!(["CanIsr_1"])
    );
    assert_eq!(
        failure["details"]["failed_checks"][0]["synchronized"],
        false
    );

    let mut diagnostic = request;
    diagnostic["enforce"] = json!(false);
    let result = ops::verify_delivery::execute(&config, &diagnostic)
        .expect("diagnostic result must remain inspectable");
    assert_eq!(result["passed"], false);
    assert_eq!(result["checks"][0]["synchronized"], false);
    assert_eq!(
        result["checks"][0]["missing_required"],
        json!(["Can_InitPortSel[0] = 1u", "CanIsr_0"])
    );
    assert_eq!(result["checks"][0]["forbidden_found"], json!(["CanIsr_1"]));
}

#[test]
fn confines_delivery_verification_to_the_declared_project_root() {
    let (root, config) = fixture();
    let wrong_root = config.tool_path.clone();
    let wrong_root_error = ops::verify_delivery::execute(
        &config,
        &json!({
            "root": wrong_root,
            "checks": [{"path": "BSWMD/Com/Com_bswmd.arxml"}]
        }),
    )
    .expect_err("verification root must contain the configured project");
    assert!(wrong_root_error
        .to_string()
        .contains("must contain the configured DaVinci project"));

    let traversal_error = ops::verify_delivery::execute(
        &config,
        &json!({
            "root": root.path(),
            "checks": [{"path": "../outside.txt"}]
        }),
    )
    .expect_err("relative path traversal must be rejected");
    assert!(traversal_error
        .to_string()
        .contains("cannot contain parent"));
}

#[test]
fn delivery_failure_limits_output_and_preserves_original_check_indices() {
    let (root, config) = fixture();
    fs::write(root.path().join("good.txt"), "content").unwrap();
    let patterns: Vec<_> = (0..12).map(|i| format!("missing\"{i}\n")).collect();
    let mut checks = vec![json!({"path": "good.txt"})];
    for i in 0..10 {
        checks.push(json!({"path": format!("missing{i}.txt"), "must_contain": patterns}));
    }
    let error =
        ops::verify_delivery::execute(&config, &json!({"root": root.path(), "checks": checks}))
            .unwrap_err();
    let failure: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
    let details = &failure["details"];
    assert_eq!(details["failed_count"], 10);
    assert_eq!(details["checks_truncated"], true);
    assert_eq!(details["failed_checks"].as_array().unwrap().len(), 8);
    assert_eq!(details["failed_checks"][0]["check_index"], 1);
    assert_eq!(details["failed_checks"][0]["missing_required_count"], 12);
    assert_eq!(
        details["failed_checks"][0]["missing_required"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        details["failed_checks"][0]["missing_required"][0],
        patterns[0]
    );
    assert!(!error.to_string().contains("good.txt"));
}

#[test]
fn unknown_module_returns_candidates_without_changing_generation_scope() {
    let (_root, config) = fixture();
    for request in [
        json!({"func":"find_module", "module":"Com_driver"}),
        json!({"func":"generate_code", "module":"Com_driver"}),
    ] {
        let error = CommandDispatcher::new()
            .dispatch_batch(&config, &request.to_string())
            .unwrap_err();
        let failure: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
        assert_eq!(failure["code"], "MODULE_NOT_FOUND");
        assert_eq!(failure["details"]["candidates"], json!(["Com"]));
    }
    assert_eq!(
        ops::find_module::execute(&config, &json!({"module_name":"com"})).unwrap()["module"],
        "Com"
    );
}

#[test]
fn diffs_ecuc_values_semantically_with_bounded_output() {
    let (_root, config) = fixture();
    let left = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let right = config
        .project_path
        .join("Config/ECUC/Test_Com_changed.arxml");
    let changed = fs::read_to_string(&left)
        .unwrap()
        .replace("<VALUE>8</VALUE>", "<VALUE>16</VALUE>")
        .replace(
            "            </PARAMETER-VALUES>",
            r#"            </PARAMETER-VALUES>
            <REFERENCE-VALUES>
              <ECUC-REFERENCE-VALUE>
                <DEFINITION-REF DEST="ECUC-REFERENCE-DEF">/MICROSAR/Com/ComConfig/ComSignal/ComTargetRef</DEFINITION-REF>
                <VALUE-REF DEST="ECUC-CONTAINER-VALUE">/Target/One</VALUE-REF>
              </ECUC-REFERENCE-VALUE>
            </REFERENCE-VALUES>"#,
        );
    fs::write(&right, changed).unwrap();

    let result = ops::diff_ecuc::execute(
        &config,
        &json!({
            "module": "com",
            "left": "Config/ECUC/Test_Com_ecuc.arxml",
            "right": right,
            "path_prefix": "Com/ComConfig/SignalA",
            "limit": 1
        }),
    )
    .unwrap();
    assert_eq!(result["total"], 2);
    assert_eq!(
        result["counts"],
        json!({"add": 1, "modify": 1, "delete": 0})
    );
    assert_eq!(result["truncated"], true);
    assert_eq!(result["changes"].as_array().unwrap().len(), 1);
    assert_eq!(result["changes"][0]["op"], "modify");
    assert_eq!(result["changes"][0]["old"], "8");
    assert_eq!(result["changes"][0]["new"], "16");

    let reverse = ops::diff_ecuc::execute(
        &config,
        &json!({"module":"Com", "left":right, "right":left, "limit":32}),
    )
    .unwrap();
    assert_eq!(
        reverse["counts"],
        json!({"add": 0, "modify": 1, "delete": 1})
    );
}

#[test]
fn diff_ecuc_rejects_duplicate_paths_and_files_outside_project() {
    let (root, config) = fixture();
    let left = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let duplicate = config
        .project_path
        .join("Config/ECUC/Test_Com_duplicate.arxml");
    let original = fs::read_to_string(&left).unwrap();
    let parameter_start = original
        .find("              <ECUC-NUMERICAL-PARAM-VALUE>")
        .unwrap();
    let relative_end = original[parameter_start..]
        .find("              </ECUC-NUMERICAL-PARAM-VALUE>")
        .unwrap();
    let parameter_end =
        parameter_start + relative_end + "              </ECUC-NUMERICAL-PARAM-VALUE>".len();
    let duplicate_block = &original[parameter_start..parameter_end];
    fs::write(
        &duplicate,
        original.replacen(
            duplicate_block,
            &format!("{duplicate_block}\n{duplicate_block}"),
            1,
        ),
    )
    .unwrap();
    let error = ops::diff_ecuc::execute(
        &config,
        &json!({"module":"Com", "left":left, "right":duplicate}),
    )
    .unwrap_err();
    let failure: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
    assert_eq!(failure["code"], "ECUC_DIFF_AMBIGUOUS_PATH");

    let outside = root.path().join("outside.arxml");
    fs::write(&outside, original).unwrap();
    assert!(ops::diff_ecuc::execute(
        &config,
        &json!({"module":"Com", "left":outside, "right":left}),
    )
    .is_err());
}

#[test]
fn sets_one_ecuc_value_semantically_and_preserves_unrelated_bytes() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let original_text = fs::read_to_string(&file).unwrap().replace('\n', "\r\n");
    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice(original_text.as_bytes());
    fs::write(&file, &original).unwrap();

    let result = ops::set_ecuc_value::execute(
        &config,
        &json!({
            "module": "com",
            "container_path": "Com/ComConfig/SignalA",
            "parameter": "ComBitPosition",
            "expected": "8",
            "value": "16"
        }),
    )
    .unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(result["path"], "Com/ComConfig/SignalA/ComBitPosition");
    assert_eq!(result["old"], "8");
    assert_eq!(result["new"], "16");
    assert_eq!(result["kind"], "parameter");

    let expected = original_text.replace("<VALUE>8</VALUE>", "<VALUE>16</VALUE>");
    let mut expected_bytes = vec![0xEF, 0xBB, 0xBF];
    expected_bytes.extend_from_slice(expected.as_bytes());
    assert_eq!(fs::read(&file).unwrap(), expected_bytes);

    let unchanged = ops::set_ecuc_value::execute(
        &config,
        &json!({
            "module": "Com",
            "container_path": "/ComConfig/SignalA",
            "parameter": "/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition",
            "expected": "16",
            "value": "16"
        }),
    )
    .unwrap();
    assert_eq!(unchanged["changed"], false);
    assert_eq!(fs::read(&file).unwrap(), expected_bytes);
}

#[test]
fn grouped_semantic_edits_preview_and_preflight_before_writing() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let original = fs::read_to_string(&file).unwrap();
    let with_reference = original.replace(
        "            </PARAMETER-VALUES>",
        r#"            </PARAMETER-VALUES>
            <REFERENCE-VALUES>
              <ECUC-REFERENCE-VALUE>
                <DEFINITION-REF DEST="ECUC-REFERENCE-DEF">/Vendor/Com/ComConfig/ComSignal/ComTargetRef</DEFINITION-REF>
                <VALUE-REF DEST="ECUC-CONTAINER-VALUE">/Target/One</VALUE-REF>
              </ECUC-REFERENCE-VALUE>
            </REFERENCE-VALUES>"#,
    );
    fs::write(&file, &with_reference).unwrap();
    let first = json!({"module":"Com", "container_path":"ComConfig/SignalA",
        "parameter":"ComBitPosition", "expected":"8", "value":"16"});
    let second = json!({"module":"Com", "container_path":"ComConfig/SignalA",
        "reference":"ComTargetRef", "expected":"/Target/One", "value":"/Target/Two"});
    let preview = json!({"func":"set_ecuc_values", "preview":true,
        "edits":[first.clone(), second.clone()]});
    CommandDispatcher::validate_batch(&config, &preview.to_string()).unwrap();
    let result = CommandDispatcher::new()
        .dispatch_batch(&config, &preview.to_string())
        .unwrap();
    assert_eq!(result["changed_files"], 1);
    assert_eq!(result["edits"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read_to_string(&file).unwrap(), with_reference);

    let stale = json!({"func":"set_ecuc_values", "edits":[first.clone(),
        {"module":"Com", "container_path":"ComConfig/SignalA",
         "reference":"ComTargetRef", "expected":"/Target/Wrong", "value":"/Target/Two"}]});
    assert!(CommandDispatcher::new()
        .dispatch_batch(&config, &stale.to_string())
        .is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), with_reference);

    let group = json!({"func":"set_ecuc_values", "edits":[first, second]});
    CommandDispatcher::new()
        .dispatch_batch(&config, &group.to_string())
        .unwrap();
    let edited = fs::read_to_string(&file).unwrap();
    assert!(edited.contains("<VALUE>16</VALUE>"));
    assert!(edited.contains(">/Target/Two</VALUE-REF>"));
    assert!(CommandDispatcher::new()
        .dispatch_batch(
            &config,
            &json!([group, {"func":"find_module", "module":"Com"}]).to_string()
        )
        .is_err());
}

#[test]
fn semantic_ecuc_edit_rejects_stale_ambiguous_and_indirect_targets() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let original = fs::read(&file).unwrap();
    let stale = ops::set_ecuc_value::execute(
        &config,
        &json!({"module":"Com", "container_path":"ComConfig/SignalA",
            "parameter":"ComBitPosition", "expected":"7", "value":"16"}),
    )
    .unwrap_err();
    let diagnostic: serde_json::Value = serde_json::from_str(&stale.to_string()).unwrap();
    assert_eq!(diagnostic["code"], "ECUC_VALUE_PRECONDITION_FAILED");
    assert_eq!(diagnostic["details"]["current"], "8");

    let indirect = ops::set_ecuc_value::execute(
        &config,
        &json!({"module":"Com", "container_path":"ComConfig",
            "parameter":"ComBitPosition", "expected":"8", "value":"16"}),
    )
    .unwrap_err();
    let diagnostic: serde_json::Value = serde_json::from_str(&indirect.to_string()).unwrap();
    assert_eq!(diagnostic["code"], "ECUC_VALUE_NOT_FOUND");
    assert_eq!(fs::read(&file).unwrap(), original);

    let text = String::from_utf8(original.clone()).unwrap();
    let start = text
        .find("          <ECUC-CONTAINER-VALUE>")
        .expect("nested container start");
    let end_marker = "          </ECUC-CONTAINER-VALUE>";
    let end = start + text[start..].find(end_marker).unwrap() + end_marker.len();
    let duplicate = format!("{}\n{}", &text[start..end], &text[start..end]);
    fs::write(&file, text.replacen(&text[start..end], &duplicate, 1)).unwrap();
    let ambiguous = ops::set_ecuc_value::execute(
        &config,
        &json!({"module":"Com", "container_path":"ComConfig/SignalA",
            "parameter":"ComBitPosition", "expected":"8", "value":"16"}),
    )
    .unwrap_err();
    let diagnostic: serde_json::Value = serde_json::from_str(&ambiguous.to_string()).unwrap();
    assert_eq!(diagnostic["code"], "ECUC_VALUE_AMBIGUOUS");
}

#[test]
fn sets_existing_ecuc_reference_semantically() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let original = fs::read_to_string(&file).unwrap();
    let with_reference = original.replace(
        "            </PARAMETER-VALUES>",
        r#"            </PARAMETER-VALUES>
            <REFERENCE-VALUES>
              <ECUC-REFERENCE-VALUE>
                <DEFINITION-REF DEST="ECUC-REFERENCE-DEF">/Vendor/Com/ComConfig/ComSignal/ComTargetRef</DEFINITION-REF>
                <VALUE-REF DEST="ECUC-CONTAINER-VALUE">/Target/One</VALUE-REF>
              </ECUC-REFERENCE-VALUE>
            </REFERENCE-VALUES>"#,
    );
    fs::write(&file, with_reference).unwrap();

    let result = ops::set_ecuc_value::execute(
        &config,
        &json!({"module":"Com", "container_path":"ComConfig/SignalA",
            "reference":"ComTargetRef", "expected":"/Target/One", "value":"/Target/Two"}),
    )
    .unwrap();
    assert_eq!(result["kind"], "reference");
    assert_eq!(result["new"], "/Target/Two");
    let edited = fs::read_to_string(&file).unwrap();
    assert!(edited.contains(">/Target/Two</VALUE-REF>"));
    assert!(!edited.contains(">/Target/One</VALUE-REF>"));
}

#[test]
fn semantic_ecuc_edit_never_treats_commented_xml_as_the_target() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let original = fs::read_to_string(&file).unwrap();
    let fake = r#"<!-- <ECUC-NUMERICAL-PARAM-VALUE>
      <DEFINITION-REF>/MICROSAR/Com/ComConfig/ComSignal/ComBitPosition</DEFINITION-REF>
      <VALUE>8</VALUE>
    </ECUC-NUMERICAL-PARAM-VALUE> -->"#;
    let content = original.replace("<VALUE>8</VALUE>", "<VALUE/>").replace(
        "            <PARAMETER-VALUES>",
        &format!("            {fake}\n            <PARAMETER-VALUES>"),
    );
    fs::write(&file, &content).unwrap();

    let error = ops::set_ecuc_value::execute(
        &config,
        &json!({"module":"Com", "container_path":"ComConfig/SignalA",
            "parameter":"ComBitPosition", "expected":"8", "value":"16"}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("parsed semantic target"));
    assert_eq!(fs::read_to_string(&file).unwrap(), content);
}

#[test]
fn semantic_ecuc_edit_is_mutating_and_doctor_only_validates() {
    let (_root, config) = fixture();
    let file = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let request = json!({"func":"set_ecuc_value", "module":"Com",
        "container_path":"ComConfig/SignalA", "parameter":"ComBitPosition",
        "expected":"8", "value":"16"});
    CommandDispatcher::validate_batch(&config, &request.to_string()).unwrap();
    assert!(fs::read_to_string(&file)
        .unwrap()
        .contains("<VALUE>8</VALUE>"));

    let batch = json!([request, {"func":"find_module", "module":"Com"}]);
    let error = CommandDispatcher::new()
        .dispatch_batch(&config, &batch.to_string())
        .unwrap_err();
    assert!(error.to_string().contains("must be standalone requests"));
    assert!(fs::read_to_string(&file)
        .unwrap()
        .contains("<VALUE>8</VALUE>"));
}

#[test]
fn doctor_rejects_missing_required_fields_before_starting_davinci() {
    let (_root, config) = fixture();
    let missing_query_module =
        CommandDispatcher::validate_batch(&config, r#"{"func":"find_module"}"#)
            .expect_err("missing query module must fail");
    assert!(missing_query_module
        .to_string()
        .contains("module is required"));

    let implicit_full_generation =
        CommandDispatcher::validate_batch(&config, r#"{"func":"generate_code"}"#)
            .expect_err("omitted generation module must not cause full generation");
    assert!(implicit_full_generation
        .to_string()
        .contains("module is required"));
    assert!(CommandDispatcher::new()
        .dispatch_batch(&config, r#"{"func":"generate_code"}"#)
        .is_err());
    assert!(CommandDispatcher::validate_batch(
        &config,
        r#"{"func":"generate_code","module":"all"}"#
    )
    .is_ok());
    assert!(CommandDispatcher::validate_batch(
        &config,
        r#"{"func":"generate_code","module_name":"Com"}"#
    )
    .is_ok());

    let mixed_inspection = CommandDispatcher::validate_batch(
        &config,
        r#"[{"func":"inspect_ecuc_containers","module":"Com"},{"func":"find_module","module":"Com"}]"#,
    )
    .expect_err("doctor must reject the same mixed batch as execution");
    assert!(mixed_inspection.to_string().contains("cannot be mixed"));
}

#[test]
fn batch_validates_every_item_before_applying_an_edit() {
    let (_root, config) = fixture();
    let file = config.project_path.join("batch-edit.arxml");
    fs::write(&file, "one\ntwo\nthree\n").expect("write batch edit file");
    let raw = json!([
        {
            "func": "edit_file",
            "path": file,
            "expected": {"2": "two"},
            "edits": {"2": "TWO"}
        },
        {"func": "find_module"}
    ])
    .to_string();

    let error = CommandDispatcher::new()
        .dispatch_batch(&config, &raw)
        .expect_err("a mutating item must reject a multi-item batch before editing");
    assert!(error.to_string().contains("must be standalone requests"));
    assert_eq!(
        fs::read_to_string(config.project_path.join("batch-edit.arxml"))
            .expect("read unmodified batch file"),
        "one\ntwo\nthree\n"
    );
}

#[test]
fn doctor_rejects_generation_inside_a_multi_item_batch() {
    let (_root, config) = fixture();
    let error = CommandDispatcher::validate_batch(
        &config,
        r#"[{"func":"find_module","module":"Com"},{"func":"generate_code","module":"Com"}]"#,
    )
    .expect_err("generation in a multi-item batch must be rejected before execution");

    assert!(error.to_string().contains("must be standalone requests"));
}

#[test]
fn edits_only_requested_line_and_preserves_crlf() {
    let (_root, config) = fixture();
    let file = config.project_path.join("edit.arxml");
    fs::write(&file, b"one\r\ntwo\r\nthree\r\n").expect("write edit file");
    let result = ops::edit_file::execute(
        &config,
        &json!({
            "path": file,
            "expected": {"2": "two"},
            "edits": {"2": "TWO"}
        }),
    )
    .expect("edit");
    assert_eq!(result["applied_edits"], 1);
    assert_eq!(
        fs::read_to_string(config.project_path.join("edit.arxml")).expect("read edited"),
        "one\r\nTWO\r\nthree\r\n"
    );
}

#[test]
fn refuses_edit_when_inspected_text_is_stale() {
    let (_root, config) = fixture();
    let file = config.project_path.join("stale.arxml");
    fs::write(&file, "one\nchanged elsewhere\nthree\n").expect("write stale file");
    let error = ops::edit_file::execute(
        &config,
        &json!({
            "path": file,
            "expected": {"2": "two"},
            "edits": {"2": "TWO"}
        }),
    )
    .expect_err("stale edit must fail");
    assert!(error.to_string().contains("file changed"));
    assert_eq!(
        fs::read_to_string(config.project_path.join("stale.arxml")).expect("read unchanged"),
        "one\nchanged elsewhere\nthree\n"
    );
}

#[test]
fn refuses_edit_outside_project() {
    let (root, config) = fixture();
    let outside = root.path().join("outside.arxml");
    fs::write(&outside, "one\n").expect("outside");
    let error = ops::edit_file::execute(
        &config,
        &json!({"path": outside, "edits": {"1": "changed"}}),
    )
    .expect_err("outside edit must fail");
    assert!(error.to_string().contains("outside project_path"));
}

#[cfg(windows)]
#[test]
fn session_paths_are_accepted_by_legacy_java_tools() {
    let (root, _config) = fixture();
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::write(
        project.join("lgk-autosar.json"),
        format!(
            "{{\"project_path\":{},\"tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("config file");

    let config = SessionConfig::load(&project).expect("load session");
    assert!(!config.project_path.to_string_lossy().starts_with(r"\\?\"));
    assert!(!config
        .dpa_file()
        .expect("dpa")
        .to_string_lossy()
        .starts_with(r"\\?\"));
}

#[test]
fn accepts_legacy_lgk_config_field_names() {
    let (root, expected) = fixture();
    let project = root.path().join("Cfg");
    let tool = root.path().join("SIP");
    fs::write(
        project.join("lgk-autosar.json"),
        format!(
            "{{\"LGK_project_path\":{},\"LGK_tool_path\":{}}}",
            serde_json::to_string(&project).expect("project JSON"),
            serde_json::to_string(&tool).expect("tool JSON")
        ),
    )
    .expect("legacy config file");

    let config = SessionConfig::load(&project).expect("load legacy session");
    assert_eq!(config.project_path, expected.project_path);
    assert_eq!(config.tool_path, expected.tool_path);
}

#[test]
fn accepts_legacy_bridge_config_filename_during_brand_migration() {
    let (root, expected) = fixture();
    let project = root.path().join("Cfg");
    fs::rename(
        project.join("lgk-autosar.json"),
        project.join("lgk-vector.json"),
    )
    .expect("rename legacy bridge config");

    let config = SessionConfig::load(&project).expect("load legacy filename");
    assert_eq!(config, expected);
}

#[test]
fn locator_reports_all_request_errors_before_reading_project() {
    let (_root, config) = fixture();
    let error = ops::locate_container::execute(
        &config,
        &json!({"query":"CanConfigSet", "path":"wrong", "limit":0}),
    )
    .unwrap_err()
    .to_string();
    let detail: serde_json::Value = serde_json::from_str(&error).unwrap();
    assert_eq!(detail["code"], "INVALID_REQUEST");
    assert_eq!(detail["details"]["issue_count"], 5);
    assert!(detail["details"]["contract"]["example"].is_object());
}

#[test]
fn locator_paginates_without_losing_total_count() {
    let (_root, config) = fixture();
    let path = config.project_path.join("Config/ECUC/Test_Com_ecuc.arxml");
    let children = (0..40).map(|i| format!("<ECUC-CONTAINER-VALUE><SHORT-NAME>S{i}</SHORT-NAME><DEFINITION-REF>/Test/Signal</DEFINITION-REF></ECUC-CONTAINER-VALUE>")).collect::<String>();
    fs::write(path, format!("<AUTOSAR>{children}</AUTOSAR>")).unwrap();
    let mut request = json!({"module":"Com", "definition_ref":"/Test/Signal"});
    let first = ops::locate_container::execute(&config, &request).unwrap();
    assert_eq!(first["count"], 40);
    assert_eq!(first["containers"].as_array().unwrap().len(), 32);
    request["offset"] = first["next_offset"].clone();
    let second = ops::locate_container::execute(&config, &request).unwrap();
    assert_eq!(second["containers"].as_array().unwrap().len(), 8);
    assert_eq!(second["containers"][0]["short_name"], "S32");
    assert_eq!(second["truncated"], false);
    assert!(second["next_offset"].is_null());
}

#[test]
fn supports_non_microsar_definitions_and_explicit_tool_selection() {
    let root = tempdir().expect("tempdir");
    let project = root.path().join("GenericCfg");
    let tool = root.path().join("VendorSip");
    let config_file = project.join("Config/ECUC/Vendor_Nm_ecuc.arxml");
    let project_file = project.join("GenericPlatform.dpa");
    let command = tool.join("DaVinci/Exec/DVCfgCmd.exe");
    fs::create_dir_all(config_file.parent().expect("config parent")).expect("project dirs");
    fs::create_dir_all(command.parent().expect("command parent")).expect("command dirs");
    fs::create_dir_all(tool.join("Definitions/Networking")).expect("definition dirs");
    fs::write(&command, b"").expect("command placeholder");
    fs::write(project.join("ArchivedProject.dpa"), "<ProjectAssistant/>").expect("other dpa");
    fs::write(
        &project_file,
        r#"<?xml version="1.0"?>
<ProjectAssistant>
  <EcucSplitter>
    <Splitter File=".\Config\ECUC\Vendor_Nm_ecuc.arxml">
      <Module Name="Nm"/>
    </Splitter>
  </EcucSplitter>
</ProjectAssistant>"#,
    )
    .expect("selected dpa");
    fs::write(
        &config_file,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <ECUC-MODULE-CONFIGURATION-VALUES>
    <SHORT-NAME>NetworkManagerInstance</SHORT-NAME>
    <DEFINITION-REF DEST="ECUC-MODULE-DEF">/AcmeAutosar/Nm</DEFINITION-REF>
  </ECUC-MODULE-CONFIGURATION-VALUES>
</AUTOSAR>"#,
    )
    .expect("module config");
    fs::write(
        tool.join("Definitions/Networking/Nm_definition.arxml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <AR-PACKAGES>
    <AR-PACKAGE>
      <SHORT-NAME>AcmeAutosar</SHORT-NAME>
      <ELEMENTS>
        <ECUC-MODULE-DEF>
          <SHORT-NAME>Nm</SHORT-NAME>
          <PARAMETERS>
            <ECUC-FLOAT-PARAM-DEF>
              <SHORT-NAME>NmMainFunctionPeriod</SHORT-NAME>
              <DEFAULT-VALUE>0.01</DEFAULT-VALUE>
            </ECUC-FLOAT-PARAM-DEF>
          </PARAMETERS>
        </ECUC-MODULE-DEF>
      </ELEMENTS>
    </AR-PACKAGE>
  </AR-PACKAGES>
</AUTOSAR>"#,
    )
    .expect("vendor definition");
    fs::write(
        project.join("lgk-autosar.json"),
        format!(
            "{{\"tool_path\":{},\"project_file\":\"GenericPlatform.dpa\",\"davinci_command_path\":\"DaVinci/Exec/DVCfgCmd.exe\"}}",
            serde_json::to_string(&tool).expect("tool JSON"),
        ),
    )
    .expect("bridge config");

    let config = SessionConfig::load(&project).expect("load generic session");
    assert_eq!(
        fs::canonicalize(config.dpa_file().expect("selected dpa")).unwrap(),
        fs::canonicalize(project_file).unwrap()
    );
    assert_eq!(
        fs::canonicalize(config.davinci_command_path.as_deref().unwrap()).unwrap(),
        fs::canonicalize(command).unwrap()
    );

    let module = ops::find_module::execute(&config, &json!({"module": "Nm"})).expect("find module");
    assert_eq!(module["definition_ref"], "/AcmeAutosar/Nm");
    let definition = ops::get_param_definition::execute(
        &config,
        &json!({"module": "Nm", "params": "NmMainFunctionPeriod"}),
    )
    .expect("vendor parameter definition");
    assert_eq!(
        definition["definitions"][0]["definition_ref"],
        "/AcmeAutosar/Nm/NmMainFunctionPeriod"
    );
}
