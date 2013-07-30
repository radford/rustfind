
extern mod syntax;
extern mod rustc;
extern mod extra;

use rustc::{front, metadata, driver, middle};
use rustc::middle::*;

use syntax::parse;
use syntax::ast;
use syntax::ast_map;
use syntax::visit;
use syntax::visit::*;
use syntax::visit::{Visitor, fn_kind};
use find_ast_node::*;
use text_formatting::*;
use syntax::diagnostic;
use syntax::codemap::BytePos;

use syntax::abi::AbiSet;
use syntax::ast;
use syntax::codemap;

use std::hashmap::*;
use std::os;
use std::local_data;
use extra::json::ToJson;

mod text_formatting;
mod find_ast_node;
mod ioutil;

pub static ctxtkey: local_data::Key<@DocContext> = &local_data::Key;

pub macro_rules! if_some {
	($b:ident in $a:expr then $c:expr)=>(
		match $a {
			Some($b)=>$c,
			None=>{}
		}
	);
}
pub macro_rules! tlogi{ 
	($($a:expr),*)=>(println((file!()+":"+line!().to_str()+": " $(+$a.to_str())*) ))
}
pub macro_rules! logi{ 
	($($a:expr),*)=>(println(""$(+$a.to_str())*) )
}
//macro_rules! dump{ ($a:expr)=>(logi!(fmt!("%s=%?",stringify!($a),$a).indent(2,160));)}
macro_rules! dump{ ($($a:expr),*)=>
	(	{	let mut txt=~""; 
			$( { txt=txt.append(
				 fmt!("%s=%?",stringify!($a),$a)+",") 
				}
			);*; 
			logi!(txt); 
		}
	)
}

pub macro_rules! if_some {
	($b:ident in $a:expr then $c:expr)=>(
		match $a {
			Some($b)=>$c,
			None=>{}
		}
	);
}

/// tags: crate,ast,parse resolve
/// Parses, resolves the given crate
fn get_ast_and_resolve(cpath: &Path, libs: ~[Path]) -> DocContext {
	


    let parsesess = parse::new_parse_sess(None);
    let sessopts = @driver::session::options {
        binary: @"rustdoc",
        maybe_sysroot: Some(@std::os::self_exe_path().get().pop()),
        addl_lib_search_paths: @mut libs,
        .. copy (*rustc::driver::session::basic_options())
    };

    let diagnostic_handler = syntax::diagnostic::mk_handler(None);
    let span_diagnostic_handler =
        syntax::diagnostic::mk_span_handler(diagnostic_handler, parsesess.cm);

    let mut sess = driver::driver::build_session_(sessopts, parsesess.cm,
                                                  syntax::diagnostic::emit,
                                                  span_diagnostic_handler);

    let (crate, tycx) = driver::driver::compile_upto(sess, sessopts.cfg.clone(),
                                                     &driver::driver::file_input(cpath.clone()),
                                                     driver::driver::cu_no_trans, None);
                                                     
	let c=crate.unwrap();
	let t=tycx.unwrap();
    DocContext { crate: c, tycx: t, sess: sess }
}


fn main() {
    use extra::getopts::*;
    use std::hashmap::HashMap;

    let args = os::args();

    let opts = ~[
        optmulti("L")
    ];

    let matches = getopts(args.tail(), opts).get();
    let libs = opt_strs(&matches, "L").map(|s| Path(*s));
	dump!(args,matches);
	dump!(libs);
    let dc = @get_ast_and_resolve(&Path(matches.free[0]), libs);
    local_data::set(ctxtkey, dc);

	debug_test(dc,matches.free[0]);
}

fn option_to_str<T:ToStr>(opt:&Option<T>)->~str {
	match *opt { Some(ref s)=>~"("+s.to_str()+~")",None=>~"(None)" }
}

trait MyToStr {  fn my_to_str(&self)->~str; }
impl MyToStr for codemap::span {
	fn my_to_str(&self)->~str { ~"("+self.lo.to_str()+~".."+self.hi.to_str() }
}

/// Todo , couldn't quite see how to declare this as a generic method of Option<T>
pub fn some<T>(o:&Option<T>,f:&fn(t:&T)) {
	match *o {
		Some(ref x)=>f(x),
		None=>{}
	}
}
pub fn some_else<T,X,Y>(o:&Option<T>,f:&fn(t:&T)->Y,default_value:Y)->Y {
	match *o {
		Some(ref x)=>f(x),
		None=>default_value
	}
}

fn debug_test(dc:&DocContext,filename:~str) {

	// TODO: parse commandline source locations,convert to codemap locations
	//dump!(ctxt.tycx);

	logi!("loading",filename);
	let source_text = ioutil::fileLoad(filename);

	logi!("==== dump def table.===")
	dump_ctxt_def_map(dc);

	logi!("==== Get table of node-spans...===")
	let node_spans=build_node_spans_table(dc.crate);
	println(node_spans_table_to_json(node_spans));

	logi!("==== Node Definition mappings...===")
	let node_def_node = build_node_def_node_table(dc);
	println(node_def_node_table_to_json(node_def_node));

	logi!("==== Test node search by location...===")
 
	// Step a test 'cursor' src_pos through the given source file..
	let mut src_pos=15 as uint;
	while src_pos<350 {
		// get the AST node under 'pos', and dump info
		let pos= text_offset_to_line_pos(source_text,src_pos);
		for pos.iter().advance |&(line,ofs)|{
			logi!(~"\n=====Find AST node at: ",src_pos," line=",line," ofs=",ofs,"=========");
			let node = find_ast_node::find(dc.crate,src_pos);
			let node_info =  find_ast_node::get_node_info_str(dc,node);
			dump!(node_info);
			// TODO - get infered type from ctxt.node_types??
			// node_id = get_node_id()
			// node_type=ctxt.node_types./*node_type_table*/.get...
			println("node ast loc:"+(do node.map |x| { option_to_str(&x.get_id()) }).to_str());
			if_some!(id in node.last().ty_node_id() then {
				dump_node_source(source_text, node_spans, id);
				if_some!(t in find_ast_node::safe_node_id_to_type(dc.tycx, id) then {
					println(fmt!("typeinfo: %?",
						{let ntt= rustc::middle::ty::get(t); ntt}));
					dump!(id,dc.tycx.def_map.find(&id));
					});
				let (def_id,opt_span)= def_span_from_node_id(dc,node_spans,id); 
				if_some!(sp in opt_span then{
					let def_line_col=text_offset_to_line_pos(source_text,*sp.lo);
					logi!("src node=",id," def node=",def_id,
						" span=",sp.my_to_str());
					dump_span(source_text, sp);
				})
			})
		}
		src_pos+=11;
	}
}


pub fn dump_node_source(text:&[u8], ns:&NodeSpans, id:ast::node_id) {
	match(ns.find(&id)) {None=>logi!("()"),
		Some(span)=>{
			dump_span(text, span);
		}
	}
}

pub fn dump_span(text:&[u8], sp:&codemap::span) {

	let line_col=text_offset_to_line_pos(text, *sp.lo);
	logi!(" line,ofs=",option_to_str(&line_col)," text=\"",
		std::str::from_bytes(text_span(text,sp)),"\"");
}

pub fn def_span_from_node_id<'a,'b>(dc:&'a DocContext, node_spans:&'b NodeSpans, id:ast::node_id)->(int,Option<&'b codemap::span>) {
	let crate_num=0;
	match dc.tycx.def_map.find(&id) { // finds a def..
		Some(a)=>{
			match get_def_id(crate_num,*a){
				Some(b)=>(b.node,node_spans.find(&b.node)),
				None=>(id as int,None)
			}
		},
		None=>(id as int,None)
	}
	
}

// see: tycx.node_types:node_type_table:HashMap<id,t>
// 't'=opaque ptr, ty::get(:t)->t_box_ to resolve it

pub fn dump_ctxt_def_map(dc:&DocContext) {
//	let a:()=ctxt.tycx.node_types
	logi!("===Test ctxt def-map table..===");
	for dc.tycx.def_map.iter().advance |(key,value)|{
		dump!(key,value);
	}
}

pub fn text_line_pos_to_offset(text:&[u8], (line,ofs_in_line):(uint,uint))->Option<uint> {
	// line as reported by grep & text editors,counted from '1' not '0'
	let mut pos = 0;
	let tlen=text.len();	
	let	mut tline=0;
	let mut line_start_pos=0;
	while pos<tlen{
		match text[pos] as char{
			'\n' => {tline+=1; line_start_pos=pos;},
//			"\a" => {tpos=0;line_pos=pos;},
			_ => {}
		}
		// todo - clamp line end
		if tline==(line-1){ 
			return Some(line_start_pos + ofs_in_line);
		}
		pos+=1;
	}
	return None;
}

pub fn text_offset_to_line_pos(text:&[u8], src_ofs:uint)->Option<(uint,uint)> {
	// line as reported by grep & text editors,counted from '1' not '0'
	let mut pos = 0;
	let tlen=text.len();	
	let	mut tline=0;
	let mut line_start_pos=0;
	while pos<tlen{
		match text[pos] as char{
			'\n' => {
				if src_ofs<=pos && src_ofs>line_start_pos {
					return Some((tline+1,src_ofs-line_start_pos));
				}
				tline+=1; line_start_pos=pos;
			},
//			"\a" => {tpos=0;line_pos=pos;},
			_ => {}
		}
		// todo - clamp line end
		pos+=1;
	}
	return None;
}

pub fn text_span<'a,'b>(text:&'a [u8],s:&'b codemap::span)->&'a[u8] {
	text.slice(*s.lo,*s.hi)
}

pub fn build_node_def_node_table(dc:&DocContext)->~HashMap<ast::node_id, ast::def_id>
{
	let mut r=~HashMap::new();
	let curr_crate_id_hack=0;	// TODO WHAT IS CRATE ID REALLY?!
	// todo .. for range(0,c.next_id) || ??
	let mut id:ast::node_id=0;
	while id<*(dc.tycx.next_id) as ast::node_id {
		if_some!(t in safe_node_id_to_type(dc.tycx,id as int) then {
			if_some!(def in dc.tycx.def_map.find(&(id as int)) then { // finds a def..
				if_some!(did in get_def_id(curr_crate_id_hack,*def) then {
					r.insert(id as ast::node_id,did);
				})
				
			});
			
			
		});
		id+=1;
	}
	r
}

pub fn def_node_id_from_node_id(dc:&DocContext, id:ast::node_id)->ast::node_id {
	let crate_num=0;	// TODO - whats crate Id really???
	match dc.tycx.def_map.find(&id) { // finds a def..
		Some(a)=>{
			match get_def_id(crate_num,*a) {
				Some(b)=>b.node,
				None=>id as int
			}
		},
		None=>(id as int)	// no definition? say its its own definition
	}
}






