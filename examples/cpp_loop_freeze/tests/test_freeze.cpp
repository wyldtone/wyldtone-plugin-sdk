// SPDX-License-Identifier: Apache-2.0
// Native smoke test for the public plugin ABI, no SDK runtime required.
#include <algorithm>
#include <cstdlib>
#define CHECK(c) do { if (!(c)) { std::fprintf(stderr, "CHECK failed: %s at line %d\n", #c, __LINE__); std::abort(); } } while(0)
#include <cmath>
#include <cstdio>
#include <vector>
#include <cstdint>

extern "C" {
  void plugin_init(int,int,int);
  int plugin_buffer_ptr();
  void plugin_process(int,int);
  int plugin_parameter_count();
  void plugin_parameter_set(int,float);
  float plugin_parameter_get(int);
}

constexpr float kPi = 3.14159265358979323846f;
static void setParam(int index,float value,float min,float max) {
  plugin_parameter_set(index, (value-min)/(max-min));
}
#if UINTPTR_MAX == UINT32_MAX
static float* shared() { return reinterpret_cast<float*>(static_cast<uintptr_t>(static_cast<uint32_t>(plugin_buffer_ptr()))); }
#endif

int main() {
#if UINTPTR_MAX > UINT32_MAX
  // Native 64-bit address truncation is inherent to the WASM32 ABI. For the
  // native test we link plugin.cpp with a special pointer-agnostic call instead.
  extern void nativeTestProcess(float*, int);
  auto render = [](std::vector<float>& block) { nativeTestProcess(block.data(), (int)block.size()/2); };
#else
  auto render = [](std::vector<float>& block) { std::copy(block.begin(), block.end(), shared()); plugin_process(plugin_buffer_ptr(), (int)block.size()); std::copy(shared(), shared()+block.size(), block.begin()); };
#endif
  plugin_init(48000,2,256);
  CHECK(plugin_parameter_count()==8);
  setParam(5, 0.0f, 0.0f, 1.0f); // dry off
  setParam(6, 1.0f, 0.0f, 1.0f); // frozen on
  setParam(1, 270.0f, 80.0f, 900.0f);
  setParam(2, 35.0f, 5.0f, 100.0f);
  setParam(3, 20.0f, 5.0f, 300.0f);
  setParam(4, 50.0f, 10.0f, 1000.0f);
  std::vector<float> block(256*2);
  for(int t=0;t<48000/256+2;t++) {
    for(int i=0;i<256;i++) {
      const int j=t*256+i;
      block[i*2] = 0.38f*std::sin(2*kPi*113.0f*j/48000.0f);
      block[i*2+1] = -0.22f*std::sin(2*kPi*127.0f*j/48000.0f);
    }
    render(block);
  }
  plugin_parameter_set(0,1.0f);
  double sumSquares=0, sumRight=0; float maxStep=0,last=0;
  for (int t=0;t<120;t++) {
    std::fill(block.begin(),block.end(),0.0f);
    render(block);
    for (int i=0;i<256;i++) {
       auto l=block[i*2],r=block[i*2+1];
       CHECK(std::isfinite(l)&&std::isfinite(r));
       if(t>20) { sumSquares+=l*l; sumRight+=r*r; maxStep=std::max(maxStep,std::abs(l-last)); }
       last=l;
    }
  }
  CHECK(sumSquares>80 && sumRight>10); // captured signal persists on silence
  CHECK(maxStep<0.2f); // no large wrap discontinuities
  plugin_parameter_set(0,0.0f);
  for(int t=0;t<50;t++) { std::fill(block.begin(),block.end(),0);render(block); }
  float peak=0;
  for(float x:block) peak=std::max(peak,std::abs(x));
  CHECK(peak<0.00001f);
  // Re-capture after release, ensuring ring recording resumes.
  for(int t=0;t<150;t++) { for(int i=0;i<256;i++) block[i*2]=block[i*2+1]=0.18f;render(block); }
  plugin_parameter_set(0,1.0f);
  for(int t=0;t<40;t++){std::fill(block.begin(),block.end(),0);render(block);}
  CHECK(block[0]>0.005f);
  std::printf("freeze native tests passed: sustain, stereo, crossfade, release, recapture\n");
}
