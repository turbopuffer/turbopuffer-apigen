// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License online at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Code generation utilities.

/// A code generation buffer.
///
/// A `CodegenBuf` provides a string-based API for generating Rust code. Its
/// value is in the various function it provides to automatically manage
/// indentation.
#[derive(Clone, Debug, Default)]
pub struct CodegenBuf {
    inner: String,
    level: usize,
}

impl CodegenBuf {
    /// Creates a new code generation buffer.
    pub fn new() -> CodegenBuf {
        CodegenBuf::default()
    }

    /// Consumes the buffer, returning its contents.
    pub fn into_string(self) -> String {
        self.inner
    }

    /// Writes a string into the buffer directly.
    pub fn write<S>(&mut self, s: S)
    where
        S: AsRef<str>,
    {
        self.inner.push_str(s.as_ref());
    }

    /// Writes a line into the buffer at the current indentation level.
    ///
    /// Specifically, the method writes (4 * indentation level) spaces into the
    /// buffer, followed by `s`, followed by a newline character.
    pub fn writeln<S>(&mut self, s: S)
    where
        S: AsRef<str>,
    {
        self.start_line();
        self.write(s);
        self.end_line();
    }

    /// Starts a new line.
    ///
    /// Specifically, the method writes (4 * indentation level) spaces into
    /// the buffer.
    pub fn start_line(&mut self) {
        for _ in 0..self.level {
            self.write("    ");
        }
    }

    /// Ends the current line.
    ///
    /// Specifically, the method writes a newline character into the buffer.
    pub fn end_line(&mut self) {
        self.write("\n");
    }

    /// Increases the indentation level by one.
    pub fn indent(&mut self) {
        self.level += 1;
    }

    /// Decreases the indentation level by one.
    pub fn unindent(&mut self) {
        self.level -= 1;
    }

    /// Writes a new indented block.
    ///
    /// Specifically, if `s` is empty, the method writes the line `{` into the
    /// buffer; otherwise writes the line `s {` into the buffer at the current
    /// indentation level. Then it increments the buffer's indentation level,
    /// runs the provided function, then decrements the indentation level and writes
    /// a closing `}`.
    pub fn write_block<S, F, R>(&mut self, s: S, f: F) -> R
    where
        S: AsRef<str>,
        F: FnOnce(&mut Self) -> R,
    {
        self.start_line();
        self.write(s.as_ref());
        if !s.as_ref().is_empty() {
            self.inner.push(' ');
        }
        self.write("{\n");
        self.level += 1;
        let ret = f(self);
        self.level -= 1;
        self.writeln("}");
        ret
    }
}
