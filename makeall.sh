#!/bin/bash

rustc lib.rs
rustc example.rs -L .
rustc channelbench.rs -L .
rustc syncchannelbench.rs -L .
rustc serverexample.rs -L .