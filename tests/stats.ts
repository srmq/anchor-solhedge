// as of
// https://stackoverflow.com/questions/5259421/cumulative-distribution-function-in-javascript
export function cdfNormal(x: number, mu: number, sigma: number): number {
    return cdfStdNormal((x-mu)/sigma);
}

export function cdfStdNormal(z: number): number {
    var j, k, kMax, m, values, total, subtotal, item, z2, z4, a, b;

    // Power series is not stable at these extreme tail scenarios
    if (z < -6) { return 0; }
    if (z >  6) { return 1; }

    m      = 1;        // m(k) == (2**k)/factorial(k)
    b      = z;        // b(k) == z ** (2*k + 1)
    z2     = z * z;    // cache of z squared
    z4     = z2 * z2;  // cache of z to the 4th
    values = [];

    // Compute the power series in groups of two terms.
    // This reduces floating point errors because the series
    // alternates between positive and negative.
    for (k=0; k<100; k+=2) {
        a = 2*k + 1;
        item = b / (a*m);
        item *= (1 - (a*z2)/((a+1)*(a+2)));
        values.push(item);
        m *= (4*(k+1)*(k+2));
        b *= z4;
    }

    // Add the smallest terms to the total first that
    // way we minimize the floating point errors.
    total = 0;
    for (k=49; k>=0; k--) {
        total += values[k];
    }

    // Multiply total by 1/sqrt(2*PI)
    // Then add 0.5 so that stdNormal(0) === 0.5
    return 0.5 + 0.3989422804014327 * total;
}

export function mean(values: number[]): number {
    const mean = (values.reduce((sum, current) => sum + current))/values.length;
    return mean;
}

export function variance(values: number[]): number {
    const average = mean(values);
    console.log(`Average of logvalues is ${average}`)

    const squareDiffs = values.map((value: number): number => {
        const diff = value - average;
        return diff*diff;
    })
    const variance = mean(squareDiffs);
    return variance;
}

export function volatilitySquared(values: number[]): number {
    let logValues = []
    for (var i = 1; i < values.length; i++) {
        logValues.push(Math.log(values[i]/values[i-1]))
    }
    //console.log('LOGVALUES')
    //console.log(JSON.stringify(logValues, null, 1))
    const result = variance(logValues)
    //console.log(`Variance is ${result}`)
    return result
}

export function convertInterest(fromInterest: number, fromGranularity: number, toGranularity: number) {
   return Math.pow(1 + fromInterest, (toGranularity/fromGranularity)) - 1
}