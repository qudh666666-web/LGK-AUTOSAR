use std::fs;

use lgk_autosar::daemon::commands::CommandDispatcher;
use lgk_autosar::ops;
use lgk_autosar::project::SessionConfig;
use serde_json::json;
use tempfile::tempdir;

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
    let result = CommandDispatcher::new().dispatch_batch(&config, &preview.to_string()).unwrap();
    assert_eq!(result["changed_files"], 1);
    assert_eq!(result["edits"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read_to_string(&file).unwrap(), with_reference);

    let stale = json!({"func":"set_ecuc_values", "edits":[first.clone(),
        {"module":"Com", "container_path":"ComConfig/SignalA",
         "reference":"ComTargetRef", "expected":"/Target/Wrong", "value":"/Target/Two"}]});
    assert!(CommandDispatcher::new().dispatch_batch(&config, &stale.to_string()).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), with_reference);

    let group = json!({"func":"set_ecuc_values", "edits":[first, second]});
    CommandDispatcher::new().dispatch_batch(&config, &group.to_string()).unwrap();
    let edited = fs::read_to_string(&file).unwrap();
    assert!(edited.contains("<VALUE>16</VALUE>"));
    assert!(edited.contains(">/Target/Two</VALUE-REF>"));
    assert!(CommandDispatcher::new().dispatch_batch(&config,
        &json!([group, {"func":"find_module", "module":"Com"}]).to_string()).is_err());
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
    assert_eq!(config.dpa_file().expect("selected dpa"), project_file);
    assert_eq!(
        config.davinci_command_path.as_deref(),
        Some(command.as_path())
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
