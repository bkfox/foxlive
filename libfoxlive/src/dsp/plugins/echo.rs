
pub struct Echo {
	fDummy: f32,
	fEntry0: f32,
	fSampleRate: i32,
	fConst0: f32,
	fEntry1: f32,
	fEntry2: f32,
	IOTA: i32,
	fRec0: [f32;2097152],
}

impl Echo {
		
	pub fn new() -> Echo { 
		Echo {
			fDummy: 0 as f32,
			fEntry0: 0.0,
			fSampleRate: 0,
			fConst0: 0.0,
			fEntry1: 0.0,
			fEntry2: 0.0,
			IOTA: 0,
			fRec0: [0.0;2097152],
		}
	}
	pub fn metadata(&mut self, m: &mut Meta) { 
		m.declare("delays.lib/name", "Faust Delay Library");
		m.declare("delays.lib/version", "0.1");
		m.declare("filename", "echo.dsp");
		m.declare("maths.lib/author", "GRAME");
		m.declare("maths.lib/copyright", "GRAME");
		m.declare("maths.lib/license", "LGPL with exception");
		m.declare("maths.lib/name", "Faust Math Library");
		m.declare("maths.lib/version", "2.1");
		m.declare("misceffects.lib/name", "Faust Math Library");
		m.declare("misceffects.lib/version", "2.0");
		m.declare("name", "echo");
	}

	pub fn getSampleRateEcho(&mut self) -> i32 {
		return self.fSampleRate;
	}
	pub fn getNumInputs(&mut self) -> i32 {
		return 1;
	}
	pub fn getNumOutputs(&mut self) -> i32 {
		return 1;
	}
	pub fn getInputRate(&mut self, channel: i32) -> i32 {
		let mut rate: i32;
		match (channel) {
			0 => {
				rate = 1;
			},
			_ => {
				rate = -1;
			},
		} 
		return rate;
	}
	pub fn getOutputRate(&mut self, channel: i32) -> i32 {
		let mut rate: i32;
		match (channel) {
			0 => {
				rate = 1;
			},
			_ => {
				rate = -1;
			},
		} 
		return rate;
	}
	
	pub fn classInit(sample_rate: i32) {
	}
	pub fn instanceResetUserInterface(&mut self) {
		self.fEntry0 = 0.5;
		self.fEntry1 = 10.0;
		self.fEntry2 = 5.0;
	}
	pub fn instanceClear(&mut self) {
		self.IOTA = 0;
		for l0 in 0..2097152 {
			self.fRec0[l0 as usize] = 0.0;
		}
	}
	pub fn instanceConstants(&mut self, sample_rate: i32) {
		self.fSampleRate = sample_rate;
		self.fConst0 = f32::min(192000.0, f32::max(1.0, (self.fSampleRate as f32)));
	}
	pub fn instanceInit(&mut self, sample_rate: i32) {
		self.instanceConstants(sample_rate);
		self.instanceResetUserInterface();
		self.instanceClear();
	}
	pub fn init(&mut self, sample_rate: i32) {
		Echo::classInit(sample_rate);
		self.instanceInit(sample_rate);
	}
	pub fn buildUserInterface(&mut self, ui_interface: &mut UI<f32>) {
		ui_interface.openVerticalBox("echo");
		ui_interface.addNumEntry("duration", &mut self.fEntry2, 5.0, 0.0, 10.0, 0.10000000000000001);
		ui_interface.addNumEntry("feedback", &mut self.fEntry0, 0.5, 0.0, 1.0, 0.01);
		ui_interface.addNumEntry("max duration", &mut self.fEntry1, 10.0, 0.0, 20.0, 0.10000000000000001);
		ui_interface.closeBox();
	}
	
	pub fn compute(&mut self, count: i32, inputs: &[&[f32]], outputs: &mut[&mut[f32]]) {
		let mut fSlow0: f32 = (self.fEntry0 as f32);
		let mut iSlow1: i32 = ((f32::min((self.fConst0 * (self.fEntry1 as f32)), f32::max(0.0, (self.fConst0 * (self.fEntry2 as f32)))) as i32) + 1);
		for i in 0..count {
			self.fRec0[(self.IOTA & 2097151) as usize] = ((inputs[0][i as usize] as f32) + (fSlow0 * self.fRec0[((self.IOTA - iSlow1) & 2097151) as usize]));
			outputs[0][i as usize] = (self.fRec0[((self.IOTA - 0) & 2097151) as usize] as f32);
			self.IOTA = (self.IOTA + 1);
		}
	}

}

