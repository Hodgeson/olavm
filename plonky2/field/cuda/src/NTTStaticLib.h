#include <cstdint> 	/* uint64_t */
#include "cuda_runtime.h"
#include "device_launch_parameters.h"

#include <stdio.h>
#include "uint128.cuh"
#include "NTT_stockham.cuh"
#include "utils.cuh"
#include "parameters.h"
#include <iostream>
#include <stdio.h>
#include <stdlib.h>
#include <fstream>
#include <string.h>
 extern "C" {
//ÿ��ntt��������������������NTTParamFBΪ�ṹ��ָ��
 void evaluate_poly(uint64_t* vec, uint64_t* result, uint64_t n, NTTParamFB* puserNTTParamFB);
 void evaluate_poly_with_offset(uint64_t* vec, uint64_t n, uint64_t domain_offset, uint64_t blowup_factor, uint64_t* result, uint64_t result_len, NTTParamFB* puserNTTParamFB);
 void interpolate_poly(uint64_t* vec, uint64_t* result, uint64_t n, NTTParamFB* puserNTTParamFB);
 void interpolate_poly_with_offset(uint64_t* vec, uint64_t* result, uint64_t n, uint64_t domain_offset, NTTParamFB* puserNTTParamFB);

 //��ʼ��GPU���� 
 void GPU_init(uint64_t n, NTTParamGroup* pNTTParamGroup);

 //������ҳ�ڴ� 
 uint64_t* Vec_init(uint64_t n);

 //�ͷ���ҳ�ڴ� 
 void Vec_free(uint64_t* Vec);
 }