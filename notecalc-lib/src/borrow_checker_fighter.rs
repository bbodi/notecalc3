use crate::helper::{AppTokens, EditorObjects, Results};
use crate::units::units::Units;
use crate::{NoteCalcApp, RenderBuckets, Variable, MAX_LINE_COUNT, SUM_VARIABLE_INDEX};
use bumpalo::Bump;

#[allow(dead_code)]
pub struct BorrowCheckerFighter {
    app_ptr: usize,
    units_ptr: usize,
    render_bucket_ptr: usize,
    tokens_ptr: usize,
    results_ptr: usize,
    vars_ptr: usize,
    editor_objects_ptr: usize,
    allocator: usize,
}

pub fn to_box_ptr<T>(t: T) -> usize {
    let ptr = Box::into_raw(Box::new(t)) as usize;
    ptr
}

pub fn create_vars() -> [Option<Variable>; MAX_LINE_COUNT + 1] {
    let mut vars = [None; MAX_LINE_COUNT + 1];
    vars[SUM_VARIABLE_INDEX] = Some(Variable {
        name: Box::from(&['s', 'u', 'm'][..]),
        value: Err(()),
    });
    return vars;
}

#[allow(dead_code)]
impl BorrowCheckerFighter {
    pub fn new(client_width: usize, client_height: usize) -> BorrowCheckerFighter {
        // put them immediately on the heap
        let editor_objects = to_box_ptr(EditorObjects::new());
        let tokens = to_box_ptr(AppTokens::new());
        let results = to_box_ptr(Results::new());
        let vars = to_box_ptr(create_vars());
        let app = to_box_ptr(NoteCalcApp::new(client_width, client_height));
        let units = to_box_ptr(Units::new());
        let render_buckets = to_box_ptr(RenderBuckets::new());
        let bumper = to_box_ptr(Bump::with_capacity(MAX_LINE_COUNT * 120));
        return BorrowCheckerFighter {
            app_ptr: app,
            units_ptr: units,
            render_bucket_ptr: render_buckets,
            tokens_ptr: tokens,
            results_ptr: results,
            vars_ptr: vars,
            editor_objects_ptr: editor_objects,
            allocator: bumper,
        };
    }

    pub fn from_ptr<'a>(ptr: usize) -> &'a mut BorrowCheckerFighter {
        let ptr_holder = unsafe { &mut *(ptr as *mut BorrowCheckerFighter) };
        return ptr_holder;
    }

    pub fn mut_app<'a>(&self) -> &'a mut NoteCalcApp {
        unsafe { &mut *(self.app_ptr as *mut NoteCalcApp) }
    }

    pub fn app<'a>(&self) -> &'a NoteCalcApp {
        unsafe { &*(self.app_ptr as *const NoteCalcApp) }
    }

    pub fn units<'a>(&self) -> &'a mut Units {
        unsafe { &mut *(self.units_ptr as *mut Units) }
    }

    pub fn mut_render_bucket<'a>(&self) -> &'a mut RenderBuckets<'a> {
        unsafe { &mut *(self.render_bucket_ptr as *mut RenderBuckets) }
    }

    pub fn render_bucket<'a>(&self) -> &'a RenderBuckets<'a> {
        unsafe { &*(self.render_bucket_ptr as *const RenderBuckets) }
    }

    pub fn mut_tokens<'a>(&self) -> &'a mut AppTokens<'a> {
        unsafe { &mut *(self.tokens_ptr as *mut AppTokens) }
    }

    pub fn tokens<'a>(&self) -> &'a AppTokens<'a> {
        unsafe { &*(self.tokens_ptr as *const AppTokens) }
    }

    pub fn mut_results<'a>(&self) -> &'a mut Results {
        unsafe { &mut *(self.results_ptr as *mut Results) }
    }

    pub fn results<'a>(&self) -> &'a Results {
        unsafe { &*(self.results_ptr as *const Results) }
    }

    pub fn editor_objects<'a>(&self) -> &'a EditorObjects {
        unsafe { &*(self.editor_objects_ptr as *const EditorObjects) }
    }

    pub fn mut_editor_objects<'a>(&self) -> &'a mut EditorObjects {
        unsafe { &mut *(self.editor_objects_ptr as *mut EditorObjects) }
    }

    pub fn mut_vars<'a>(&self) -> &'a mut [Option<Variable>] {
        unsafe { &mut (&mut *(self.vars_ptr as *mut [Option<Variable>; MAX_LINE_COUNT + 1]))[..] }
    }

    pub fn vars<'a>(&self) -> &'a [Option<Variable>] {
        unsafe { &(&*(self.vars_ptr as *const [Option<Variable>; MAX_LINE_COUNT + 1]))[..] }
    }

    pub fn allocator<'a>(&self) -> &'a Bump {
        unsafe { &*(self.allocator as *const Bump) }
    }

    pub fn mut_allocator<'a>(&self) -> &'a mut Bump {
        unsafe { &mut *(self.allocator as *mut Bump) }
    }
}
