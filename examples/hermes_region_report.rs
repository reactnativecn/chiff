use chiff::{parse_artifact, HermesFunctionInfoBlock, HermesSectionKind};
use std::{env, fs, process};

const HEADER_LEN: usize = 128;

#[derive(Debug, Clone, Copy)]
struct RegionDelta {
    old_len: usize,
    new_len: usize,
    common_prefix_len: usize,
    common_suffix_len: usize,
}

impl RegionDelta {
    fn changed_old_len(self) -> usize {
        self.old_len
            .saturating_sub(self.common_prefix_len + self.common_suffix_len)
    }

    fn changed_new_len(self) -> usize {
        self.new_len
            .saturating_sub(self.common_prefix_len + self.common_suffix_len)
    }

    fn changed_span(self) -> usize {
        self.changed_old_len().max(self.changed_new_len())
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(old_path) = args.next() else {
        print_usage_and_exit();
    };
    let Some(new_path) = args.next() else {
        print_usage_and_exit();
    };
    if args.next().is_some() {
        print_usage_and_exit();
    }

    let old = fs::read(&old_path).unwrap_or_else(|error| {
        eprintln!("failed to read old file {}: {}", old_path, error);
        process::exit(1);
    });
    let new = fs::read(&new_path).unwrap_or_else(|error| {
        eprintln!("failed to read new file {}: {}", new_path, error);
        process::exit(1);
    });

    let old_artifact = parse_artifact(&old).unwrap_or_else(|| {
        eprintln!("failed to parse old Hermes artifact");
        process::exit(1);
    });
    let new_artifact = parse_artifact(&new).unwrap_or_else(|| {
        eprintln!("failed to parse new Hermes artifact");
        process::exit(1);
    });

    println!(
        "header version={} form={:?}",
        old_artifact.header.version, old_artifact.header.form
    );
    println!(
        "file sizes old={} new={}",
        old_artifact.payload_len, new_artifact.payload_len
    );
    println!(
        "function_layout={}",
        old_artifact.function_layout.is_some() && new_artifact.function_layout.is_some()
    );
    print_delta("header", region_delta(&old[..HEADER_LEN], &new[..HEADER_LEN]));

    println!();
    println!("sections");
    for section in &old_artifact.section_layout.sections {
        let old_range = section.offset as usize..(section.offset + section.len) as usize;
        let new_range = new_artifact
            .section_layout
            .sections
            .iter()
            .find(|candidate| candidate.kind == section.kind)
            .map(|candidate| candidate.offset as usize..(candidate.offset + candidate.len) as usize)
            .unwrap_or(0..0);

        let delta = region_delta(&old[old_range], &new[new_range]);
        print_delta(section_kind_name(section.kind), delta);
    }

    if let (Some(old_functions), Some(new_functions)) = (
        old_artifact.function_layout.as_ref(),
        new_artifact.function_layout.as_ref(),
    ) {
        println!();
        println!("gaps");
        print_delta(
            "structured_to_bytecode",
            region_delta(
                &old[old_artifact.section_layout.structured_end_offset as usize
                    ..old_functions.bytecode_region_start as usize],
                &new[new_artifact.section_layout.structured_end_offset as usize
                    ..new_functions.bytecode_region_start as usize],
            ),
        );
        print_delta(
            "bytecode_to_info",
            region_delta(
                &old[old_functions.bytecode_region_end as usize
                    ..old_artifact.header.debug_info_offset as usize],
                &new[new_functions.bytecode_region_end as usize
                    ..new_artifact.header.debug_info_offset as usize],
            ),
        );
        print_delta(
            "debug_info",
            region_delta(
                &old[old_artifact.header.debug_info_offset as usize
                    ..old_artifact.header.file_length as usize],
                &new[new_artifact.header.debug_info_offset as usize
                    ..new_artifact.header.file_length as usize],
            ),
        );

        println!();
        println!("top changed functions");
        let mut functions = Vec::new();
        for index in 0..old_functions.functions.len().max(new_functions.functions.len()) {
            let old_range = old_functions
                .functions
                .get(index)
                .map(|function| function.bytecode_offset as usize..function.body_end_offset as usize)
                .unwrap_or(0..0);
            let new_range = new_functions
                .functions
                .get(index)
                .map(|function| function.bytecode_offset as usize..function.body_end_offset as usize)
                .unwrap_or(0..0);
            let delta = region_delta(&old[old_range], &new[new_range]);
            if delta.changed_span() > 0 {
                functions.push((index, delta));
            }
        }
        functions.sort_by_key(|(_, delta)| std::cmp::Reverse(delta.changed_span()));
        for (index, delta) in functions.into_iter().take(10) {
            print_delta(&format!("function[{index}]"), delta);
        }

        println!();
        println!("top changed info blocks");
        let mut info_blocks = Vec::new();
        for index in 0..old_functions
            .info_blocks
            .len()
            .max(new_functions.info_blocks.len())
        {
            let old_block = old_functions.info_blocks.get(index);
            let new_block = new_functions.info_blocks.get(index);
            let delta = region_delta_for_info_block(&old, &new, old_block, new_block);
            if delta.changed_span() > 0 {
                info_blocks.push((index, delta));
            }
        }
        info_blocks.sort_by_key(|(_, delta)| std::cmp::Reverse(delta.changed_span()));
        for (index, delta) in info_blocks.into_iter().take(10) {
            print_delta(&format!("info[{index}]"), delta);
        }
    }
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage: cargo run --example hermes_region_report -- <old-hbc> <new-hbc>");
    process::exit(2);
}

fn print_delta(label: &str, delta: RegionDelta) {
    println!(
        "{label}: old_len={} new_len={} prefix={} suffix={} changed_old={} changed_new={}",
        delta.old_len,
        delta.new_len,
        delta.common_prefix_len,
        delta.common_suffix_len,
        delta.changed_old_len(),
        delta.changed_new_len(),
    );
}

fn region_delta(old: &[u8], new: &[u8]) -> RegionDelta {
    let common_prefix_len = common_prefix_len(old, new);
    let common_suffix_len = common_suffix_len(old, new, common_prefix_len);

    RegionDelta {
        old_len: old.len(),
        new_len: new.len(),
        common_prefix_len,
        common_suffix_len,
    }
}

fn region_delta_for_info_block(
    old: &[u8],
    new: &[u8],
    old_block: Option<&HermesFunctionInfoBlock>,
    new_block: Option<&HermesFunctionInfoBlock>,
) -> RegionDelta {
    let old_range = old_block
        .map(|block| block.offset as usize..block.end_offset as usize)
        .unwrap_or(0..0);
    let new_range = new_block
        .map(|block| block.offset as usize..block.end_offset as usize)
        .unwrap_or(0..0);
    region_delta(&old[old_range], &new[new_range])
}

fn common_prefix_len(old: &[u8], new: &[u8]) -> usize {
    old.iter()
        .zip(new.iter())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}

fn common_suffix_len(old: &[u8], new: &[u8], prefix_len: usize) -> usize {
    let old_remaining = old.len().saturating_sub(prefix_len);
    let new_remaining = new.len().saturating_sub(prefix_len);
    let max_suffix_len = old_remaining.min(new_remaining);

    old[old.len().saturating_sub(max_suffix_len)..]
        .iter()
        .rev()
        .zip(new[new.len().saturating_sub(max_suffix_len)..].iter().rev())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}

fn section_kind_name(kind: HermesSectionKind) -> &'static str {
    match kind {
        HermesSectionKind::FunctionHeaders => "function_headers",
        HermesSectionKind::StringKinds => "string_kinds",
        HermesSectionKind::IdentifierHashes => "identifier_hashes",
        HermesSectionKind::SmallStringTable => "small_string_table",
        HermesSectionKind::OverflowStringTable => "overflow_string_table",
        HermesSectionKind::StringStorage => "string_storage",
        HermesSectionKind::LiteralValueBuffer => "literal_value_buffer",
        HermesSectionKind::ObjectKeyBuffer => "object_key_buffer",
        HermesSectionKind::ObjectShapeTable => "object_shape_table",
        HermesSectionKind::BigIntTable => "big_int_table",
        HermesSectionKind::BigIntStorage => "big_int_storage",
        HermesSectionKind::RegExpTable => "regexp_table",
        HermesSectionKind::RegExpStorage => "regexp_storage",
        HermesSectionKind::CjsModuleTable => "cjs_module_table",
        HermesSectionKind::FunctionSourceTable => "function_source_table",
    }
}
